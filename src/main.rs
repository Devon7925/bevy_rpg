//! A simplified implementation of the classic game "Breakout".

use std::{
    collections::HashMap, env, fmt::{self, Formatter}, thread, time::Duration
};

use bevy::{
    a11y::accesskit::TextAlign, prelude::*, tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task}, text::Text2dBounds, time::common_conditions::on_timer, transform::commands, utils::HashSet
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use serde::{Deserialize, Serialize};

const CHARACTER_SCALE: Vec3 = Vec3::new(0.2, 0.2, 0.0);
const PADDLE_SPEED: f32 = 500.0;

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
}

#[derive(Component)]
struct NPC {
    backstory: String,
    chat_cooldown: f32,
    history: Vec<(String, Action)>,
    state: NPCState,
    property: Option<Rect>,
}

impl NPC {
    const CHAT_COOLDOWN: f32 = 400.0;
}

impl Default for NPC {
    fn default() -> Self {
        NPC {
            backstory: "".to_string(),
            chat_cooldown: 20.0,
            history: vec![],
            state: NPCState::Idle,
            property: None,
        }
    }
}

#[derive(Component)]
struct DialogRequest(Task<Option<String>>);

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

// Add the game's entities to our world
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    // Background
    let background_scale = 2.0;
    commands.spawn(SpriteBundle {
        texture: asset_server.load("textures/background.png"),
        transform: Transform {
            translation: Vec3::new(-1500.0, 1500.0, 0.0),
            scale: Vec3::new(background_scale, background_scale, 0.0),
            ..default()
        },
        ..Default::default()
    });

    // Plants
    for x in 0..=14 {
        for y in 0..=10 {
            commands.spawn((
                Plant::default(),
                SpriteBundle {
                    texture: asset_server.load("textures/plants/stage1.png"),
                    transform: Transform {
                        translation: Vec3::new(
                            -700.0 + 60.0 * (x as f32),
                            0.0 + 60.0 * (y as f32),
                            0.0,
                        ),
                        scale: Vec3::new(0.3, 0.3, 0.0),
                        ..default()
                    },
                    ..Default::default()
                },
            ));
        }
    }

    for x in 0..=23 {
        for y in 0..=23 {
            commands.spawn((
                Plant::default(),
                SpriteBundle {
                    texture: asset_server.load("textures/plants/stage1.png"),
                    transform: Transform {
                        translation: Vec3::new(
                            -2400.0 + 60.0 * (x as f32),
                            0.0 + 60.0 * (y as f32),
                            0.0,
                        ),
                        scale: Vec3::new(0.3, 0.3, 0.0),
                        ..default()
                    },
                    ..Default::default()
                },
            ));
        }
    }

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
            backstory: "You are Theo. A stern 16th century Farmer living in a small village in medieval europe. You live with your wife Jessica and son Jeff on your own small patch of land. You know your land is small but it has been owned by centuries by your family. Jeff wants to start working on your neighbor Bill's land because it is much bigger, but you want your family to continue farming your ancestral land. You also know you are getting old and tired and will soon need Jeff's help, especially if you have to support Jessica without help. You speak in short dialog.".to_string(),
            state: NPCState::Farming,
            property: Some(Rect::new(-700.0, 0.0, 14.0*60.0 - 700.0, 10.0*60.0).inset(1.0)),
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
            backstory: "You are Jeff. A 16th century Farmer living in a small village in medival europe. You currently live with your parents Theo and Jessica on their small farm. However you know your land is small and will have trouble feeding all three of you so you'd like to move to your neighbor Bill's land in order to stop burdening your family. You've brought this up before, but Theo object due to heritage reasons, whereas you think eating is more important than tradition. You speak in short dialog.".to_string(),
            chat_cooldown: 40.0,
            state: NPCState::Farming,
            property: Some(Rect::new(-700.0, 0.0, 14.0*60.0 - 700.0, 10.0*60.0).inset(1.0)),
            ..Default::default()
        },
    )).add(fill_character);
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
    let (mut paddle_transform, mut character) = query.single_mut();
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
    let new_paddle_position = Vec2::new(
        paddle_transform.translation.x,
        paddle_transform.translation.y,
    ) + direction * PADDLE_SPEED * time.delta_seconds();

    // Update the paddle position,

    paddle_transform.translation.x = new_paddle_position.x;
    paddle_transform.translation.y = new_paddle_position.y;

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
                < 300.0
            {
                character.actions.iter().for_each(|action| {
                    npc.history.push((character.name.clone(), action.clone()));
                });
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAIMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}
#[derive(Serialize, Deserialize, Debug)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    temperature: f32,
    max_tokens: u32,
    top_p: f32,
    frequency_penalty: f32,
    presence_penalty: f32,
    stop: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

fn update_npcs(
    time: Res<Time>,
    mut npc_query: Query<(Entity, &mut NPC, &Character, &Transform)>,
    character_query: Query<(&Character, &Transform)>,
    mut commands: Commands,
) {
    let thread_pool = AsyncComputeTaskPool::get();
    for (npc_entity_id, mut npc, character, npc_location) in &mut npc_query {
        if npc.chat_cooldown > 0.0 {
            npc.chat_cooldown -= time.delta_seconds();
        } else {
            npc.chat_cooldown = NPC::CHAT_COOLDOWN;

            let name = character.name.clone();

            let nearby_people = character_query
                .iter()
                .filter(|(_, character_transform)| {
                    character_transform
                        .translation
                        .distance(npc_location.translation)
                        < 300.0
                })
                .map(|(character, _)| character.name.clone())
                .collect::<Vec<String>>();
            let nearby_people = if nearby_people.len() > 0 {
                format!("You see {} near you.", nearby_people.join(", "))
            } else {
                "".to_string()
            };

            let mut messages = vec![OpenAIMessage {
                role: "system".to_string(),
                content: format!(
                    "You speak in the format '{name}: Dialog'. {} {nearby_people}",
                    npc.backstory
                ),
                name: None,
            }];

            let mut prev_is_chatter_assistant = false;
            let mut current_content = "".to_string();
            for (chatter, action) in npc.history.iter() {
                let is_chatter_assistant = chatter == &character.name;
                if prev_is_chatter_assistant != is_chatter_assistant && current_content.len() > 0 {
                    messages.push(OpenAIMessage {
                        role: if prev_is_chatter_assistant {
                            "assistant"
                        } else {
                            "user"
                        }
                        .to_string(),
                        content: current_content.trim().to_string(),
                        name: None,
                    });
                    current_content = "".to_string();
                }
                prev_is_chatter_assistant = is_chatter_assistant;
                current_content = format!("{}{}", current_content, action.get_context(chatter));
            }
            if current_content.len() > 0 {
                messages.push(OpenAIMessage {
                    role: if prev_is_chatter_assistant {
                        "assistant"
                    } else {
                        "user"
                    }
                    .to_string(),
                    content: current_content.trim().to_string(),
                    name: None,
                });
            }

            let character_name = character.name.clone();

            let task = thread_pool.spawn(async_compat::Compat::new(async move {
                let request_body = OpenAIRequest {
                    model: "gpt-3.5-turbo".to_string(),
                    messages,
                    temperature: 1.0,
                    max_tokens: 64,
                    top_p: 1.0,
                    frequency_penalty: 0.0,
                    presence_penalty: 0.0,
                    stop: "\"".to_string(),
                };
    
                println!("Request body: {:?}", request_body);
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
                let request_text = response.text().await.unwrap();
                let res: OpenAIResponse = match serde_json::from_str(&request_text) {
                    Ok(res) => res,
                    Err(e) => {
                        println!("Could not parse response: {}", request_text);
                        panic!("Error: {:?}", e);
                    }
                };
                let character_response = res.choices[0].message.content.clone();
                character_response
                    .strip_prefix(format!("{}: ", character_name).as_str())
                    .map(|s| s.to_string())
            }));
            commands.entity(npc_entity_id).insert(DialogRequest(task));
        }
    }
}

fn handle_npc_dialog_requests(
    mut npcs: Query<(Entity, &mut Character, &mut DialogRequest)>,
    mut commands: Commands,
) {
    for (entity, mut character, mut task) in &mut npcs {
        if let Some(mut commands_queue) = future::block_on(future::poll_once(&mut task.0)) {
            // append the returned command queue to have it execute later
            if let Some(response) = commands_queue.take() {
                character.actions.push(Action::Talk(response));
            }
            commands.entity(entity).remove::<DialogRequest>();
        }
    }
}

fn update_farmers(
    mut query: Query<(&NPC, &mut Character, &mut Transform), Without<Plant>>,
    plants: Query<(&Transform, &Plant)>,
    time: Res<Time>,
) {
    for (npc, mut character, mut npc_transform) in &mut query {
        if matches!(npc.state, NPCState::Farming) {
            let mut closest_plant = None;
            let mut closest_distance = f32::INFINITY;
            for (plant_transform, plant) in &plants {
                if let Some(property) = npc.property {
                    if !property.contains(plant_transform.translation.xy()) {
                        continue;
                    }
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
                let new_position =
                    npc_transform.translation + direction.normalize() * 50.0 * time.delta_seconds();
                npc_transform.translation = new_position;
                if closest_distance < Plant::HARVEST_RANGE {
                    character.actions.push(Action::Harvest);
                }
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
    for player_transform in player_query.iter() {
        for mut camera_transform in &mut camera_query.iter_mut() {
            if camera_transform
                .translation
                .distance(player_transform.translation)
                > 100.0
            {
                camera_transform.translation = player_transform.translation
                    + (camera_transform.translation - player_transform.translation).normalize()
                        * 100.0;
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
