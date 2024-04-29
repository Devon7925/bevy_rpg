//! A simplified implementation of the classic game "Breakout".

use std::{
    collections::HashMap,
    env,
    fmt::{self, Formatter},
};

use bevy::{
    prelude::*,
    tasks::{futures_lite::future, AsyncComputeTaskPool, Task},
    text::Text2dBounds,
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

const CHARACTER_SCALE: Vec3 = Vec3::new(0.2, 0.2, 0.0);
const CHARACTER_SPEED: f32 = 150.0;

const BACKGROUND_COLOR: Color = Color::rgb(0.9, 0.9, 0.9);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .add_systems(Startup, setup)
        // Add our gameplay simulation systems to the fixed timestep schedule
        // which runs at 64 Hz by default
        .add_systems(
            Update,
            (
                (
                    player_input,
                    update_npcs,
                    handle_npc_dialog_requests,
                    update_farmers,
                    update_travelers,
                    camera_follow_player,
                    update_plants,
                    inventory_update,
                    update_saturation,
                ),
                update_history,
                handle_actions,
            )
                .chain(),
        )
        .add_systems(Update, (ui_system, bevy::window::close_on_esc))
        .run();
}

#[derive(Eq, PartialEq, Hash, Clone)]
enum Item {
    Plant,
    Meat,
}

impl Item {
    fn saturation(&self) -> f32 {
        match self {
            Item::Plant => 10.0,
            Item::Meat => 20.0,
        }
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Item::Plant => write!(f, "Plant"),
            Item::Meat => write!(f, "Meat"),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
enum Action {
    Eat,
    Harvest,
    Talk(String),
}

impl Action {
    fn get_context(&self, actor: &str) -> String {
        match self {
            Action::Eat => format!("{} eats something. ", actor),
            Action::Harvest => format!("{} harvests. ", actor),
            Action::Talk(speech) => format!("{} says \"{}\". ", actor, speech),
        }
    }
}

#[derive(Component)]
struct Character {
    name: String,
    items: Vec<(Item, u32)>,
    saturation: f32,
    actions: Vec<Action>,
}

impl Default for Character {
    fn default() -> Self {
        Character {
            name: "".to_string(),
            items: vec![],
            saturation: 100.0,
            actions: vec![],
        }
    }
}

#[derive(Component)]
struct Player {
    text_box: String,
}

enum NPCState {
    Idle,
    Farming,
    Traveling(String),
}

impl NPCState {
    fn get_context(&self) -> String {
        match self {
            NPCState::Idle => "You are currently idle.".to_string(),
            NPCState::Farming => "You are currently farming.".to_string(),
            NPCState::Traveling(destination) => format!("You are currently traveling to {}. ", destination),
        }
    }
}

#[derive(Component)]
struct NPC {
    backstory: String,
    chat_cooldown: f32,
    history: Vec<(String, Action)>,
    state: NPCState,
}

impl NPC {
    const CHAT_COOLDOWN: f32 = 80.0;
}

impl Default for NPC {
    fn default() -> Self {
        NPC {
            backstory: "".to_string(),
            chat_cooldown: NPC::CHAT_COOLDOWN / 2.0,
            history: vec![],
            state: NPCState::Idle,
        }
    }
}

#[derive(Component)]
struct DialogRequest(Task<Option<OpenAIMessage>>);

#[derive(Component, Deref, DerefMut)]
struct StartPos(Vec2);

#[derive(Component, Deref, DerefMut)]
struct Plant {
    growth: f32,
}

impl Default for Plant {
    fn default() -> Self {
        Plant { growth: 0.0 }
    }
}

impl Plant {
    const HARVEST_RANGE: f32 = 50.0;

    fn is_grown(&self) -> bool {
        self.growth >= 1.0
    }

    fn get_growth_stage(&self) -> u32 {
        (self.growth * 3.0).floor() as u32
    }

    fn grow(&mut self, amount: f32) {
        self.growth += amount;
        if self.growth > 1.0 {
            self.growth = 1.0;
        }
    }
}

#[derive(Component)]
struct Region {
    name: String,
    range: Rect,
}

// Add the game's entities to our world
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    // Background
    let background_scale = 2.0;
    commands.spawn(SpriteBundle {
        texture: asset_server.load("textures/background.png"),
        transform: Transform {
            translation: Vec3::new(-1500.0, 1500.0, -1.0),
            scale: Vec3::new(background_scale, background_scale, 0.0),
            ..default()
        },
        ..Default::default()
    });

    // Regions & Plants
    let theo_farm_rect = Rect::new(-700.0, 0.0, 140.0, 600.0);
    commands.spawn((Region {
        name: "Theo's Family Farm".to_string(),
        range: theo_farm_rect,
    },));
    fill_rect_with_plants(&mut commands, &asset_server, theo_farm_rect);

    let bill_farm_rect = Rect::new(-2400.0, 0.0, -1020.0, 1380.0);
    commands.spawn((Region {
        name: "Bill's Farm".to_string(),
        range: bill_farm_rect,
    },));
    fill_rect_with_plants(&mut commands, &asset_server, bill_farm_rect);

    // Player
    commands
        .spawn((
            StartPos(Vec2::new(0.0, 0.0)),
            Character {
                name: "James".to_string(),
                ..Default::default()
            },
            Player {
                text_box: "".to_string(),
            },
        ))
        .add(fill_character);

    // NPCs
    commands.spawn((
        StartPos(Vec2::new(-100.0, 80.0)),
        Character {
            name: "Theo".to_string(),
            ..Default::default()
        },
        NPC {
            backstory: "You are Theo. A stern 16th century Farmer living in a small village in medieval europe. You live with your wife Jessica and son Jeff on your own small patch of land. You know your land is small but it has been owned by centuries by your family. Jeff wants to start working on your neighbor Bill's land because it is much bigger, but you want your family to continue farming your ancestral land. You also know you are getting old and tired and will soon need Jeff's help, especially if you have to support Jessica without help. ".to_string(),
            state: NPCState::Farming,
            ..Default::default()
        },
    )).add(fill_character);

    commands.spawn((
        StartPos(Vec2::new(100.0, 50.0)),
        Character {
            name: "Jeff".to_string(),
            ..Default::default()
        },
        NPC {
            backstory: "You are Jeff. A young 16th century Farmer living in a small village in medival europe. You currently live with your parents Theo and Jessica on their small farm. However you know your land is small and will have trouble feeding all three of you so you'd like to move to your neighbor Bill's land in order to stop burdening your family. You've brought this up before, but Theo objects due to heritage reasons, whereas you think eating is more important than tradition. ".to_string(),
            chat_cooldown: 10.0,
            state: NPCState::Idle,
            ..Default::default()
        },
    )).add(fill_character);

    commands.spawn((
        StartPos(bill_farm_rect.center()),
        Character {
            name: "Bill".to_string(),
            ..Default::default()
        },
        NPC {
            backstory: "You are Bill. A cunning 16th century Farmer living in a small village in medival europe. You live on a farm you've been growing in size for decades. You hope to recruit a village boy Jeff from a nearby farm to help you farm your land, as it currently takes up most of your time. ".to_string(),
            chat_cooldown: 30.0,
            state: NPCState::Farming,
            ..Default::default()
        },
    )).add(fill_character);
}

fn fill_rect_with_plants(commands: &mut Commands, asset_server: &Res<AssetServer>, rect: Rect) {
    for x in (rect.min.x as i32..=rect.max.x as i32).step_by(60) {
        for y in (rect.min.y as i32..=rect.max.y as i32).step_by(60) {
            commands.spawn((
                Plant::default(),
                SpriteBundle {
                    texture: asset_server.load("textures/plants/stage1.png"),
                    transform: Transform {
                        translation: Vec3::new(x as f32, y as f32, 0.0),
                        scale: Vec3::new(0.3, 0.3, 0.0),
                        ..default()
                    },
                    ..Default::default()
                },
            ));
        }
    }
}

fn fill_character(mut entity: EntityWorldMut<'_>) {
    let start_pos = entity
        .get::<StartPos>()
        .unwrap_or(&StartPos(Vec2::new(0.0, 0.0)))
        .0;
    let char_name = entity.get::<Character>().unwrap().name.clone();
    let texture = entity.world_scope(|world| {
        let asset_server = world.get_resource::<AssetServer>().unwrap();
        asset_server.load(format!("textures/characters/{}.png", char_name))
    });
    entity.insert(SpriteBundle {
        texture,
        transform: Transform {
            translation: start_pos.extend(0.0),
            scale: CHARACTER_SCALE,
            ..default()
        },
        ..default()
    });
    let text_child_id = entity.world_scope(|world| {
        let asset_server = world.get_resource::<AssetServer>().unwrap();

        let font = asset_server.load("fonts/FiraSans-Bold.ttf");
        let text_style = TextStyle {
            font: font.clone(),
            font_size: 12.0,
            color: Color::BLACK,
        };
        let text_alignment = JustifyText::Center;
        world
            .spawn((Text2dBundle {
                text: Text::from_section("", text_style.clone()).with_justify(text_alignment),
                transform: Transform {
                    translation: Vec3::new(0.0, -200.0, 10.0),
                    scale: Vec3::new(5.0, 5.0, 0.0),
                    ..default()
                },
                text_2d_bounds: Text2dBounds {
                    size: Vec2::new(200.0, 200.0),
                    ..default()
                },
                text_anchor: bevy::sprite::Anchor::TopCenter,
                ..default()
            },))
            .id()
    });
    entity.add_child(text_child_id);
}

fn player_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &mut Character), With<Player>>,
    time: Res<Time>,
) {
    let Ok((mut transform, mut character)) = query.get_single_mut() else {
        return;
    };
    let mut direction = Vec2::new(0.0, 0.0);

    if keyboard_input.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }

    if keyboard_input.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    if keyboard_input.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }

    if keyboard_input.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }

    // Calculate the new horizontal paddle position based on player input
    let new_paddle_position = Vec2::new(transform.translation.x, transform.translation.y)
        + direction * CHARACTER_SPEED * time.delta_seconds();

    // Update the paddle position,

    transform.translation.x = new_paddle_position.x;
    transform.translation.y = new_paddle_position.y;

    if keyboard_input.just_pressed(KeyCode::Space) {
        character.actions.push(Action::Harvest);
    }
}

fn update_history(
    mut npc_query: Query<(&mut NPC, &Transform)>,
    character_query: Query<(&Character, &Transform)>,
) {
    for (mut npc, npc_transform) in &mut npc_query {
        for (character, character_transform) in &character_query {
            if npc_transform
                .translation
                .distance(character_transform.translation)
                < 600.0
            {
                character.actions.iter().for_each(|action| {
                    npc.history.push((character.name.clone(), action.clone()));
                });
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAIToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIToolFunction,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAIRequest {
    messages: Vec<OpenAIMessage>,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    logit_bias: Option<HashMap<u32, f32>>,
    temperature: f32,
    max_tokens: u32,
    top_p: f32,
    frequency_penalty: f32,
    presence_penalty: f32,
    stop: Vec<String>,
    tools: Vec<OpenAITool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionCall,
}
#[derive(Serialize, Deserialize, Debug)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAIError {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAIErrorResponse {
    error: OpenAIError,
}

fn update_npcs(
    time: Res<Time>,
    mut npc_query: Query<(Entity, &mut NPC, &Character, &Transform)>,
    character_query: Query<(&Character, &Transform)>,
    region_query: Query<&Region>,
    mut commands: Commands,
) {
    let thread_pool = AsyncComputeTaskPool::get();
    for (npc_entity_id, mut npc, character, npc_location) in &mut npc_query {
        if npc.chat_cooldown > 0.0 {
            npc.chat_cooldown -= time.delta_seconds();
        } else {
            npc.chat_cooldown = NPC::CHAT_COOLDOWN;

            let name = character.name.clone();

            let mut nearby_people = character_query
                .iter()
                .filter(|(_, character_transform)| {
                    character_transform
                        .translation
                        .distance(npc_location.translation)
                        < 300.0
                })
                .filter(|(nearby_character, _)| nearby_character.name != name)
                .map(|(nearby_character, _)| nearby_character.name.clone())
                .collect::<Vec<String>>();

            // correctly formatted a list of names and store to nearby_people with "and" in between the last two names
            let nearby_people = if nearby_people.len() > 0 {
                let last_person = nearby_people.pop().unwrap();
                if nearby_people.len() > 0 {
                    format!(
                        "You see {} and {} near you. ",
                        nearby_people.join(", "),
                        last_person
                    )
                } else {
                    format!("You see {} near you. ", last_person)
                }
            } else {
                "".to_string()
            }; 
            

            let mut active_regions = region_query
                .iter()
                .filter(|region| region.range.contains(npc_location.translation.xy()))
                .map(|region| region.name.clone())
                .collect::<Vec<String>>();
            
            let active_regions = if active_regions.len() > 0 {
                let last_region = active_regions.pop().unwrap();
                if active_regions.len() > 0 {
                    format!(
                        "You are currently in {} and {}. ",
                        active_regions.join(", "),
                        last_region
                    )
                } else {
                    format!("You are currently in {}. ", last_region)
                }
            } else {
                "".to_string()
            };

            let mut messages = vec![
                OpenAIMessage {
                    role: "system".to_string(),
                    content: Some(format!(
                        "You are playing the role of an npc in a video game. You will be given a large amount of context and should either come up with a short response from your character in the format '{name}: Dialog', or call a function to change your behavior. ",
                    )),
                    tool_calls: None,
                    name: None,
                },
            ];

            let current_task = npc.state.get_context();
            let current_content = format!(
                "{}{}{active_regions}{nearby_people}{current_task}",
                npc.backstory,
                npc.history
                    .iter()
                    .unique()
                    .map(|(chatter, action)| action.get_context(chatter))
                    .join("")
            );
            if current_content.len() > 0 {
                messages.push(OpenAIMessage {
                    role: "user".to_string(),
                    content: Some(current_content.trim().to_string()),
                    tool_calls: None,
                    name: None,
                });
            }

            let task = thread_pool.spawn(async_compat::Compat::new(async move {
                let request_body = OpenAIRequest {
                    messages,
                    model: "gpt-3.5-turbo".to_string(),
                    logit_bias: Some([(9, -5.0)].iter().cloned().collect()),
                    temperature: 1.0,
                    max_tokens: 64,
                    top_p: 1.0,
                    frequency_penalty: 0.0,
                    presence_penalty: 0.0,
                    stop: vec!["\n".to_string()],
                    tools: vec![OpenAITool {
                        tool_type: "function".to_string(),
                        function: OpenAIToolFunction {
                            name: "set_task".to_string(),
                            description: "Change what you are currently doing. destination parameter should be used when task is traveling".to_string(),
                            parameters: serde_json::json!({
                                "type": "object",
                                "properties": {
                                    "task": {"type": "string", "enum": ["idle", "farming", "traveling"]},
                                    "destination": {"type": "string", "enum": ["Theo's Family Farm", "Bill's Farm"]},
                                },
                                "required": ["task"],
                            }),
                        },
                    }],
                };

                println!("Request body: {:?}", serde_json::to_string(&request_body));
                let key = "OPENAI_API_KEY";
                let token = env::var(key).unwrap();

                let client = reqwest::Client::new();
                let response = client
                    .post("https://api.openai.com/v1/chat/completions")
                    .bearer_auth(token)
                    .json(&request_body)
                    .send()
                    .await
                    .unwrap();
                let response_text = response.text().await.unwrap();
                let res: OpenAIResponse = match serde_json::from_str(&response_text) {
                    Ok(res) => res,
                    Err(e) => {
                        if let Ok(_) = serde_json::from_str::<OpenAIErrorResponse>(&response_text) {
                            println!("Error: {:?}", response_text);
                            return None;
                        } else {
                            println!("Could not parse response: {}", response_text);
                            panic!("Error: {:?}", e);
                        }
                    }
                };
                println!("Response: {:?}", response_text);
                Some(res.choices[0].message.clone())
            }));
            commands.entity(npc_entity_id).insert(DialogRequest(task));
        }
    }
}

fn handle_npc_dialog_requests(
    mut npcs: Query<(Entity, &mut NPC, &mut Character, &mut DialogRequest)>,
    mut commands: Commands,
) {
    for (entity, mut npc, mut character, mut task) in &mut npcs {
        if let Some(mut commands_queue) = future::block_on(future::poll_once(&mut task.0)) {
            // append the returned command queue to have it execute later
            if let Some(message) = commands_queue.take() {
                if let Some(character_response) = message.content.clone() {
                    if let Some(character_response) = character_response
                        .strip_prefix(format!("{}: ", character.name).as_str())
                        .map(|s| s.to_string())
                    {
                        println!("Response: {} says {}", character.name, character_response);
                        character.actions.push(Action::Talk(character_response));
                    }
                };
                if let Some(tool_calls) = message.tool_calls {
                    for tool_call in tool_calls {
                        match tool_call.function.name.as_str() {
                            "set_task" => {
                                println!("Task arguments: {}", tool_call.function.arguments);
                                if let Some(task_args) = serde_json::from_str::<serde_json::Value>(
                                    tool_call.function.arguments.as_str(),
                                )
                                .ok() {
                                    if let Some(task) = task_args["task"].as_str().map(|s| s.to_string())
                                    {
                                        npc.state = match task.as_str() {
                                            "idle" => NPCState::Idle,
                                            "farming" => NPCState::Farming,
                                            "traveling" => if let Some(destination) = task_args["destination"].as_str().map(|s| s.to_string()) {
                                                NPCState::Traveling(destination)
                                            } else {
                                                println!("Invalid destination: {}", tool_call.function.arguments);
                                                NPCState::Idle
                                            },
                                            invalid_state => {
                                                println!("Invalid state: {}", invalid_state);
                                                NPCState::Idle
                                            }
                                        }
                                    } else {
                                        println!(
                                            "Invalid task arguments: {}",
                                            tool_call.function.arguments.clone()
                                        );
                                    }
                                } else {
                                    println!("Invalid task arguments: {}", tool_call.function.arguments);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            commands.entity(entity).remove::<DialogRequest>();
        }
    }
}

fn update_farmers(
    mut query: Query<(&NPC, &mut Character, &mut Transform), Without<Plant>>,
    plants: Query<(&Transform, &Plant)>,
    regions: Query<&Region>,
    time: Res<Time>,
) {
    for (npc, mut character, mut npc_transform) in &mut query {
        if matches!(npc.state, NPCState::Farming) {
            let mut closest_plant = None;
            let mut closest_distance = f32::INFINITY;
            for (plant_transform, plant) in &plants {
                let mut is_in_valid_region = false;
                for region in &regions {
                    if region.range.contains(npc_transform.translation.xy())
                        && region.range.contains(plant_transform.translation.xy())
                    {
                        is_in_valid_region = true;
                    }
                }
                if !is_in_valid_region {
                    continue;
                }
                let distance = plant_transform
                    .translation
                    .distance(npc_transform.translation);
                if distance < closest_distance && plant.is_grown() {
                    closest_distance = distance;
                    closest_plant = Some(plant_transform.translation);
                }
            }
            if let Some(plant_position) = closest_plant {
                let direction = plant_position - npc_transform.translation;
                let new_position = npc_transform.translation
                    + direction.normalize() * CHARACTER_SPEED * time.delta_seconds();
                npc_transform.translation = new_position;
                if closest_distance < Plant::HARVEST_RANGE {
                    character.actions.push(Action::Harvest);
                }
            }
        }
    }
}

fn update_travelers(
    mut query: Query<(&mut NPC, &mut Transform), Without<Plant>>,
    regions: Query<&Region>,
    time: Res<Time>,
) {
    for (mut npc, mut npc_transform) in &mut query {
        if let NPCState::Traveling(destination) = &npc.state {
            let destination_region = regions
                .iter()
                .find(|region| region.name == *destination)
                .unwrap();
            if !destination_region.range.contains(npc_transform.translation.xy()) {
                let direction = destination_region.range.center() - npc_transform.translation.xy();
                let new_position = npc_transform.translation
                    + direction.normalize().extend(0.0) * CHARACTER_SPEED * time.delta_seconds();
                npc_transform.translation = new_position;
            } else {
                npc.state = NPCState::Idle;
            }
        }
    }

}

fn ui_system(mut contexts: EguiContexts, mut players: Query<(&mut Player, &mut Character)>) {
    for (mut player, mut character) in &mut players {
        egui::Window::new("Chat box").show(contexts.ctx_mut(), |ui| {
            ui.add(egui::ProgressBar::new(character.saturation / 100.0).text("Saturation"));
            ui.label("Inventory");
            for (item, count) in &character.items {
                ui.label(format!("{}: {}", item, count));
            }
            ui.text_edit_singleline(&mut player.text_box);
            if ui.button("Submit").clicked() {
                character
                    .actions
                    .push(Action::Talk(player.text_box.clone()));
                player.text_box = "".to_string();
            }
        });
    }
}

fn camera_follow_player(
    player_query: Query<&Transform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<Player>)>,
) {
    const CAMERA_MAX_DISTANCE: f32 = 200.0;
    for player_transform in player_query.iter() {
        for mut camera_transform in &mut camera_query.iter_mut() {
            if camera_transform
                .translation
                .distance(player_transform.translation)
                > CAMERA_MAX_DISTANCE
            {
                camera_transform.translation = player_transform.translation
                    + (camera_transform.translation - player_transform.translation).normalize()
                        * CAMERA_MAX_DISTANCE;
            }
        }
    }
}

fn update_plants(
    mut query: Query<(&mut Plant, &mut Handle<Image>)>,
    asset_server: Res<AssetServer>,
) {
    for (mut plant, mut texture) in &mut query {
        plant.grow(0.001);
        *texture = asset_server.load::<Image>(format!(
            "textures/plants/stage{}.png",
            plant.get_growth_stage()
        ));
    }
}

// stack items and auto eat if saturation is low
fn inventory_update(mut query: Query<&mut Character>) {
    for mut character in &mut query.iter_mut() {
        let mut new_items = HashMap::new();
        for (item, count) in character.items.clone() {
            let new_count = new_items.entry(item).or_insert(0);
            *new_count += count;
        }
        character.items.clear();
        for (item, count) in new_items {
            character.items.push((item, count));
        }
        // remove empty items
        character.items.retain(|(_, count)| *count > 0);
    }
}

fn update_saturation(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Character)>,
    time: Res<Time>,
) {
    for (entity, mut character) in &mut query.iter_mut() {
        character.saturation -= time.delta_seconds();
        if character.saturation < 0.0 {
            commands.entity(entity).despawn();
        } else if character.saturation < 30.0 {
            if character
                .items
                .iter()
                .any(|(item, _)| item.saturation() > 0.0)
            {
                character.actions.push(Action::Eat);
            }
        }
    }
}

fn handle_actions(
    mut query: Query<(&Transform, &mut Character, &Children)>,
    mut plants: Query<(&Transform, &mut Plant)>,
    mut text_query: Query<&mut Text>,
) {
    for (character_transform, mut character, children) in &mut query.iter_mut() {
        for action in character.actions.clone() {
            match action {
                Action::Eat => {
                    for (item, count) in &mut character.items {
                        if item.saturation() > 0.0 {
                            *count -= 1;
                            character.saturation += item.saturation();
                            break;
                        }
                    }
                }
                Action::Harvest => {
                    for (plant_transform, mut plant) in &mut plants {
                        if plant_transform
                            .translation
                            .distance(character_transform.translation)
                            < Plant::HARVEST_RANGE
                        {
                            if plant.is_grown() {
                                character.items.push((Item::Plant, 1));
                                plant.growth = 0.0;
                            }
                        }
                    }
                }
                Action::Talk(speech) => {
                    for &child in children.iter() {
                        text_query.get_mut(child).unwrap().sections[0].value = speech.clone();
                    }
                }
            }
        }
        character.actions.clear();
    }
}
