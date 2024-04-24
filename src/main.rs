//! A simplified implementation of the classic game "Breakout".

use std::{
    collections::HashMap,
    env,
    fmt::{self, Formatter},
};

use bevy::{
    a11y::accesskit::TextAlign, prelude::*, text::Text2dBounds
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
            FixedUpdate,
            (
                update_chat_history,
                update_speech_box,
                apply_velocity,
                move_player,
                move_npc,
                harvest_plant,
                update_npcs,
                update_farmers,
                camera_follow_player,
                update_plants,
                inventory_update,
                update_saturation,
            )
                // `chain`ing systems together runs them in order
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

impl fmt::Display for Item {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Item::Plant => write!(f, "Plant"),
            Item::Meat => write!(f, "Meat"),
        }
    }
}
#[derive(Component)]
struct Character {
    name: String,
    speech: Option<String>,
    items: Vec<(Item, u32)>,
    saturation: f32,
}

impl Default for Character {
    fn default() -> Self {
        Character {
            name: "".to_string(),
            speech: None,
            items: vec![],
            saturation: 100.0,
        }
    }
}

#[derive(Component)]
struct Player {
    text_box: String,
}

enum NPCState {
    Idle,
    Farming
}

#[derive(Component)]
struct NPC {
    backstory: String,
    chat_cooldown: f32,
    chat_history: Vec<(String, String)>,
    state: NPCState,
    property: Option<Rect>,
}

impl Default for NPC {
    fn default() -> Self {
        NPC {
            backstory: "".to_string(),
            chat_cooldown: 20.0,
            chat_history: vec![],
            state: NPCState::Idle,
            property: None,
        }
    }
}

#[derive(Component, Deref, DerefMut)]
struct StartPos(Vec2);

#[derive(Component, Deref, DerefMut)]
struct Velocity(Vec2);

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

    fn get_harvest_range(&self) -> f32 {
        match self.get_growth_stage() {
            0 => 0.0,
            1 => 10.0,
            2 => 30.0,
            _ => 50.0,
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
                text: Text::from_section("", text_style.clone())
                    .with_justify(text_alignment),
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

fn move_player(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<Player>>,
    time: Res<Time>,
) {
    let mut paddle_transform = query.single_mut();
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
}

fn move_npc(
    mut query: Query<(&mut Transform, &mut NPC), Without<Plant>>,
    plants: Query<(&Transform, &Plant)>,
    time: Res<Time>,
) {
    for (mut transform, npc) in &mut query.iter_mut() {
        match npc.state {
            NPCState::Idle => {
            }
            NPCState::Farming => {
                let mut closest_plant = None;
                let mut closest_distance = f32::INFINITY;
                for (plant_transform, plant) in &plants {
                    if let Some(property) = npc.property {
                        if !property.contains(plant_transform.translation.xy()) {
                            continue;
                        }
                    }
                    let distance = plant_transform.translation.distance(transform.translation);
                    if distance < closest_distance && plant.is_grown() {
                        closest_distance = distance;
                        closest_plant = Some(plant_transform.translation);
                    }
                }
                if let Some(plant_position) = closest_plant {
                    let direction = plant_position - transform.translation;
                    let new_position = transform.translation + direction.normalize() * 50.0 * time.delta_seconds();
                    transform.translation = new_position;
                }
            }
        }
    }
}

fn harvest_plant(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&Transform, &mut Character), (With<Player>, Without<Plant>)>,
    mut plant_query: Query<(&Transform, &mut Plant)>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        for (player_transform, mut player) in &mut query {
            for (plant_transform, mut plant) in &mut plant_query {
                if plant_transform
                    .translation
                    .distance(player_transform.translation)
                    < plant.get_harvest_range() && plant.is_grown()
                {
                    player.items.push((Item::Plant, 1));
                    plant.growth = 0.0;
                }
            }
        }
    }
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_seconds();
        transform.translation.y += velocity.y * time.delta_seconds();
    }
}

fn update_speech_box(
    mut character_query: Query<(&mut Character, &Children)>,
    mut text_query: Query<&mut Text>,
) {
    for (mut character, children) in character_query.iter_mut() {
        // `children` is a collection of Entity IDs
        for &child in children.iter() {
            if let Some(speech) = character.speech.clone() {
                println!("{}: {}", character.name, speech);
                text_query.get_mut(child).unwrap().sections[0].value = speech;
            }
        }
        character.speech = None;
    }
}

fn update_chat_history(
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
                if let Some(speech) = &character.speech {
                    npc.chat_history
                        .push((character.name.clone(), speech.clone()));
                }
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

fn update_npcs(time: Res<Time>, mut npc_query: Query<(&mut NPC, &mut Character)>) {
    for (mut npc, mut character) in &mut npc_query {
        if npc.chat_cooldown > 0.0 {
            npc.chat_cooldown -= time.delta_seconds();
        } else {
            npc.chat_cooldown = 400.0;
            let mut messages = vec![OpenAIMessage {
                role: "system".to_string(),
                content: npc.backstory.clone(),
                name: None,
            }];
            let mut prev_is_chatter_assistant = false;
            let mut current_content = "".to_string();
            for (chatter, dialog) in npc.chat_history.iter() {
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
                current_content = format!("{}\n{}: {}", current_content, chatter, dialog);
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
            let request_body = OpenAIRequest {
                model: "gpt-3.5-turbo".to_string(),
                messages,
                temperature: 1.0,
                max_tokens: 64,
                top_p: 1.0,
                frequency_penalty: 0.0,
                presence_penalty: 0.0,
                stop: "\n".to_string(),
            };

            println!("Request body: {:?}", request_body);
            let key = "OPENAI_API_KEY";
            let token = env::var(key).unwrap();

            let client = reqwest::blocking::Client::new();
            let response = client
                .post("https://api.openai.com/v1/chat/completions")
                .bearer_auth(token)
                .json(&request_body)
                .send()
                .unwrap();
            let request_text = response.text().unwrap();
            let res: OpenAIResponse = match serde_json::from_str(&request_text) {
                Ok(res) => res,
                Err(e) => {
                    println!("Could not parse response: {}", request_text);
                    panic!("Error: {:?}", e);
                }
            };
            let character_response = res.choices[0].message.content.clone();
            println!("{}'s Response: {}", character.name, character_response);
            character.speech = character_response
                .strip_prefix(format!("{}: ", character.name).as_str())
                .map(|s| s.to_string());
        }
    }
}

fn update_farmers(mut query: Query<(&mut NPC, &mut Character, &Transform), Without<Plant>>, mut plant_query: Query<(&Transform, &mut Plant)>) {
    for (mut npc, mut character, farmer_transform) in &mut query.iter_mut() {
        if matches!(npc.state, NPCState::Farming) {
            for (plant_transform, mut plant) in &mut plant_query.iter_mut() {
                if plant_transform.translation.distance(farmer_transform.translation) < plant.get_harvest_range() && plant.is_grown() {
                    character.items.push((Item::Plant, 1));
                    plant.growth = 0.0;
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
                character.speech = Some(player.text_box.clone());
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
        *texture = asset_server.load::<Image>(format!("textures/plants/stage{}.png", plant.get_growth_stage()));
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

        if character.saturation < 30.0 {
            let mut new_saturation = character.saturation;
            for (item, count) in character.items.iter_mut() {
                let provided_saturation = match item {
                    Item::Plant => 10.0,
                    Item::Meat => 20.0,
                };
                if provided_saturation > 0.0 {
                    while *count > 0 && new_saturation < 60.0 {
                        new_saturation += provided_saturation;
                        *count -= 1;
                    }
                }
            }
            character.saturation = new_saturation;
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
        }
    }
}
