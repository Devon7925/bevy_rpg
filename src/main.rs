//! A simplified implementation of the classic game "Breakout".

use bevy::{
    prelude::*,
    sprite::collide_aabb::{collide, Collision},
    sprite::MaterialMesh2dBundle,
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};

// These constants are defined in `Transform` units.
// Using the default 2D camera they correspond 1:1 with screen pixels.
const PADDLE_SIZE: Vec3 = Vec3::new(20.0, 20.0, 0.0);
const PADDLE_SPEED: f32 = 500.0;

const BACKGROUND_COLOR: Color = Color::rgb(0.9, 0.9, 0.9);
const PADDLE_COLOR: Color = Color::rgb(0.3, 0.3, 0.7);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .add_event::<CollisionEvent>()
        .add_systems(Startup, setup)
        // Add our gameplay simulation systems to the fixed timestep schedule
        // which runs at 64 Hz by default
        .add_systems(
            FixedUpdate,
            (apply_velocity, move_paddle, ui_example_system)
                // `chain`ing systems together runs them in order
                .chain(),
        )
        .add_systems(Update, (update_speech, bevy::window::close_on_esc))
        .run();
}

#[derive(Component)]
struct Character {
    speech: Option<String>,
}

#[derive(Component)]
struct Player {
    text_box: String,
}

#[derive(Component)]
struct NPC;

#[derive(Component, Deref, DerefMut)]
struct Velocity(Vec2);

#[derive(Component)]
struct Collider;

#[derive(Event, Default)]
struct CollisionEvent;

#[derive(Resource)]
struct CollisionSound(Handle<AudioSource>);

// Add the game's entities to our world
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    // Player

    commands.spawn((
        Character {
            speech: Some("Hi im the player".to_string()),
        },
        Player {
            text_box: "".to_string(),
        },
        Collider,
    )).add(fill_character);

    // NPCs
    commands.spawn((
        Character {
            speech: Some("Hi im bob".to_string()),
        },
        NPC,
        Collider,
    )).add(fill_character);

    commands.spawn((
        Character {
            speech: Some("Hi im bill".to_string()),
        },
        NPC,
        Collider,
    )).add(fill_character);
}

fn fill_character(mut entity: EntityWorldMut<'_>) {
    entity.insert(SpriteBundle {
        transform: Transform {
            translation: Vec3::new(0.0, 0.0, 0.0),
            scale: PADDLE_SIZE,
            ..default()
        },
        sprite: Sprite {
            color: PADDLE_COLOR,
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
        let text_alignment = TextAlignment::Center;
        world.spawn((
            Text2dBundle {
                text: Text::from_section("", text_style.clone())
                    .with_alignment(text_alignment),
                transform: Transform {
                    translation: Vec3::new(0.0, 2.0, 0.0),
                    scale: Vec3::new(0.05, 0.05, 0.0),
                    ..default()
                },
                ..default()
            },
        )).id()
    });
    entity.add_child(text_child_id);
}

fn move_paddle(
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<&mut Transform, With<Player>>,
    time: Res<Time>,
) {
    let mut paddle_transform = query.single_mut();
    let mut direction = Vec2::new(0.0, 0.0);

    if keyboard_input.pressed(KeyCode::Left) {
        direction.x -= 1.0;
    }

    if keyboard_input.pressed(KeyCode::Right) {
        direction.x += 1.0;
    }

    if keyboard_input.pressed(KeyCode::Up) {
        direction.y += 1.0;
    }

    if keyboard_input.pressed(KeyCode::Down) {
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

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_seconds();
        transform.translation.y += velocity.y * time.delta_seconds();
    }
}

fn update_speech(character_query: Query<(&Character, &Children)>, mut text_query: Query<&mut Text>) {
    for (character, children) in character_query.iter() {
        // `children` is a collection of Entity IDs
        for &child in children.iter() {
            // get the health of each child unit
            text_query.get_mut(child).unwrap().sections[0].value = character.speech.clone().unwrap_or("".to_string());

            // do something
        }
    }
}

fn ui_example_system(mut contexts: EguiContexts, mut players: Query<(&mut Player, &mut Character)>) {
    for (mut player, mut character) in &mut players {
        egui::Window::new("Chat box").show(contexts.ctx_mut(), |ui| {
            ui.text_edit_singleline(&mut player.text_box);
            if ui.button("Submit").clicked() {
                character.speech = Some(player.text_box.clone());
                player.text_box = "".to_string();
            }
        });
    }
}
