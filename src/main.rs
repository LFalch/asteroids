use self_compare::SliceCompareExt;

use rand::Rng;

use bevy::{
    prelude::*,
    tasks::{ComputeTaskPool, ParallelIterator},
    input::system::exit_on_esc_system,
    render::pass::ClearColor,
    sprite::collide_aabb::{collide, Collision},
};

fn main() {
    App::build()
        .add_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
        .add_resource(WindowDescriptor {
            title: "asteroids".to_owned(),
            .. Default::default()
        })
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup.system())
        .add_system(player_movement_system.system())
        .add_system(physics_movement.system())
        .add_system(collision_system.system())
        .add_system(exit_on_esc_system.system())
        .add_system(asteroid_spawner_system.system())
        .add_system(scoreboard_text_system.system())
        .add_system(restart_key_system.system())
        .add_system(bullet_life_system.system())
        .run();
}

fn setup(
    commands: &mut Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
) {
    commands
        .spawn(Camera2dBundle::default())
        .spawn(CameraUiBundle::default())
        .spawn(SpriteBundle {
            material: materials.add(Color::rgb(0.5, 0.5, 1.0).into()),
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
            sprite: Sprite::new(Vec2::new(32.0, 32.0)),
            ..Default::default()
        })
        .with(Physics {
            velocity: Vec3::new(0., 0., 0.),
            mass: 100.,
        })
        .with(PlayerShip{lives: 4})
        .spawn(TextBundle {
            text: Text {
                font: asset_server.load("DroidSansMono.ttf"),
                value: "Score:".to_string(),
                style: TextStyle {
                    color: Color::rgb(0.5, 0.5, 1.0),
                    font_size: 40.0,
                    ..Default::default()
                },
            },
            style: Style {
                position_type: PositionType::Absolute,
                position: Rect {
                    top: Val::Px(5.0),
                    left: Val::Px(5.0),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        })
        .with(Scoreboard { score: 0 })
        .spawn((AsteroidSpawner { timer: ASTEROID_SPAWN_TIME * 3. -0.5, .. Default::default() },))
        ;
}

const ACCELERATION: f32 = 256.+DECELERATION;
// TODO: make deceleration related to the square of the speed, like air resistance. It feels better, and will set a max speed.
const DECELERATION: f32 = 96.;
const ROTATION_RATE: f32 = 210. * std::f32::consts::PI / 180.;

fn player_movement_system(
    commands: &mut Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    time: Res<Time>,
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<(&mut Physics, &mut Transform, &PlayerShip)>,
) {
    for (mut physics, mut transform, player) in query.iter_mut() {
        let mut direction = 0.0;
        if keyboard_input.pressed(KeyCode::Left) || keyboard_input.pressed(KeyCode::A) {
            direction += 1.0;
        }

        if keyboard_input.pressed(KeyCode::Right) || keyboard_input.pressed(KeyCode::D) {
            direction -= 1.0;
        }

        let rotation = &mut transform.rotation;

        *rotation *= Quat::from_rotation_z(direction * ROTATION_RATE * time.delta_seconds());

        let mut acceleration = 0.0;
        if keyboard_input.pressed(KeyCode::Down) || keyboard_input.pressed(KeyCode::S) {
            acceleration -= ACCELERATION;
        }

        if keyboard_input.pressed(KeyCode::Up) || keyboard_input.pressed(KeyCode::W) {
            acceleration += ACCELERATION;
        }

        physics.velocity += *rotation * Vec3::new(acceleration * time.delta_seconds(), 0., 0.);


        let dspeed = DECELERATION * time.delta_seconds();

        let speed = physics.velocity.length();
        if speed <= dspeed {
            physics.velocity = Vec3::zero();
        } else {
            let velocity = &mut physics.velocity;
            *velocity -= velocity.normalize() * dspeed;
        }

        
        if player.lives > 0 && keyboard_input.just_pressed(KeyCode::Space) {
            let dir = transform.rotation * Vec3::unit_x();
            spawn_bullet(commands, &mut materials, transform.translation + 32. * dir, physics.velocity + 512. * dir);
        }
    }
}

fn physics_movement(
    pool: Res<ComputeTaskPool>,
    time: Res<Time>,
    windows: Res<Windows>,
    mut query: Query<(&Physics, &mut Transform)>,
) {
    let window = windows.get_primary().expect("No primary window.");
    let width = window.width();
    let height = window.height();

    query.par_iter_mut(32).for_each(&pool, |(physics, mut transform)| {
        let translation = &mut transform.translation;

        *translation += physics.velocity * time.delta_seconds();

        // Messy code to keep inside frame
        *translation += Vec3::new(width * 1.5, height * 1.5, 0.);
        translation.x %= width;
        translation.y %= height;
        *translation -= Vec3::new(width * 0.5, height * 0.5, 0.);
    })
}

const ASTEROID_LIMIT: usize = 256;
const ASTEROID_SPAWN_TIME: f32 = 3.;

fn asteroid_spawner_system(
    commands: &mut Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    time: Res<Time>,
    windows: Res<Windows>,
    mut query: Query<(Entity, &mut AsteroidSpawner)>,
    asteroids: Query<&Asteroid>,
) {
    let window = windows.get_primary().expect("No primary window.");
    let w = 0.5 * window.width();
    let h = 0.5 * window.height();

    if asteroids.iter().len() >= ASTEROID_LIMIT {
        return
    }

    let mut rnd = rand::thread_rng();
    for (entity, mut spawner) in query.iter_mut() {
        spawner.timer += time.delta_seconds();

        while spawner.timer >= ASTEROID_SPAWN_TIME {
            spawner.timer -= ASTEROID_SPAWN_TIME;
            for _ in 0..spawner.amount {
                let sign = if rnd.gen::<bool>() { 1. } else { -1. };
                let sign2 = if rnd.gen::<bool>() { 1. } else { -1. };
                let (x, y, svx, svy) = if rnd.gen::<bool>() {
                    (rnd.gen_range(-w ..= w), sign * h, sign2, -sign)
                } else {
                    (sign * w, rnd.gen_range(-h ..= h), -sign, sign2)
                };
                spawn_asteroid(commands, &mut materials, x, y, svx * rnd.gen_range(0. ..= 300.), svy * rnd.gen_range(0. ..= 250.), rnd.gen_range(16. ..= 128.), true);
            }
            if spawner.one_time {
                commands.remove_one::<AsteroidSpawner>(entity);
                break
            }
        }
    }
}

fn scoreboard_text_system(mut query: Query<(&mut Text, &Scoreboard)>, player_query: Query<&PlayerShip>) {
    for (mut text, scoreboard) in query.iter_mut() {
        text.value = format!("Score: {}", scoreboard.score);

        for PlayerShip{lives} in player_query.iter() {
            text.value += &format!("\nLives: {}", lives);
        }
    }
}

#[derive(Debug)]
struct PlayerShip {
    lives: i8,
}

#[derive(Debug)]
struct AsteroidSpawner {
    one_time: bool,
    amount: usize,
    timer: f32,
}

impl Default for AsteroidSpawner {
    fn default() -> Self {
        AsteroidSpawner {
            one_time: false,
            amount: 1,
            timer: 0.,
        }
    }
}

#[derive(Debug)]
struct Asteroid;

#[derive(Debug, Default)]
struct Bullet {
    lifetime: f32,
}

#[derive(Debug)]
struct Scoreboard {
    score: i32,
}

#[derive(Debug)]
struct Physics {
    velocity: Vec3,
    mass: f32,
}

fn spawn_bullet(c: &mut Commands, materials: &mut Assets<ColorMaterial>, pos: Vec3, v: Vec3) {
    c.spawn(SpriteBundle {
        material: materials.add(Color::rgb(0.7, 0.7, 0.1).into()),
        transform: Transform::from_translation(pos),
        sprite: Sprite::new(Vec2::new(8.0, 8.0)),
        ..Default::default()
    })
    .with( Physics {
        velocity: v,
        mass: 10.0,
    })
    .with(AsteroidSpawner{amount: 3, timer: ASTEROID_SPAWN_TIME-BULLET_LIFE+1.0, .. Default::default()})
    .with(Bullet::default());
}

fn spawn_asteroid(c: &mut Commands, materials: &mut Assets<ColorMaterial>, x: f32, y: f32, vx: f32, vy: f32, mass: f32, spawner: bool) {
    let size = mass.sqrt() * 12.;
    c.spawn(SpriteBundle {
        material: materials.add(Color::rgb(0.7, 0.7, 0.7).into()),
        transform: Transform::from_translation(Vec3::new(x, y, 0.0)),
        sprite: Sprite::new(Vec2::new(size, size)),
        ..Default::default()
    })
    .with(Physics {
        velocity: Vec3::new(vx, vy, 0.),
        mass,
    })
    .with(Asteroid);
    if spawner {
        c.with(AsteroidSpawner { one_time: true, .. Default::default() });
    }
}

fn collision_system(
    commands: &mut Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut query: Query<(Entity, Option<&mut PlayerShip>, Option<&Bullet>, Option<&Asteroid>, &mut Physics, &Transform, &Sprite)>,
    mut scoreboard_query: Query<&mut Scoreboard>,
) {
    let mut ents: Vec<_> = query.iter_mut().collect();
    ents.compare_self_mut(|
        (left_entity, ref mut left_pl, left_bul, left_ast, ref mut left_phys, left_trans, left_spr),
        (right_entity, ref mut right_pl, right_bul, right_ast, ref mut right_phys, right_trans, right_spr)
    | {
        let collision = collide(
            left_trans.translation,
            left_spr.size,
            right_trans.translation,
            right_spr.size,
        );
        if let Some(collision) = collision {
            let resolve = match ((left_pl, left_bul, left_ast, (&left_phys, left_trans)), (right_pl, right_bul, right_ast, (&right_phys, right_trans))) {
                ((None, None, Some(_), _), (None, None, Some(_), _)) => {
                    // ast <-> ast
                    true
                }
                ((None, Some(_), None, _), (None, Some(_), None, _)) => true,
                ((Some(ref mut pl), None, None, _), (None, None, Some(_), _)) | ((None, None, Some(_), _), (Some(ref mut pl), None, None, _)) => {
                    pl.lives = pl.lives.saturating_sub(1);
                    true
                }
                ((None, Some(_), None, _), (None, None, Some(_), (ast_phys, ast_trans))) | ((None, None, Some(_), (ast_phys, ast_trans)), (None, Some(_), None, _)) => {
                    if ast_phys.mass > 32. {
                        let (x, y) = (ast_trans.translation.x, ast_trans.translation.y);
                        let side = ast_phys.velocity.normalize().cross(Vec3::unit_z()) * 50.;
                        let mass = ast_phys.mass / 2.;
                        let phys = ast_phys.velocity + side;
                        let phys2 = ast_phys.velocity - side;
                        spawn_asteroid(commands, &mut materials, x, y, phys.x, phys.y, mass, false);
                        spawn_asteroid(commands, &mut materials, x, y, phys2.x, phys2.y, mass, false);
                    }

                    commands
                        .despawn(*left_entity)
                        .despawn(*right_entity);

                    for mut scoreboard in scoreboard_query.iter_mut() {
                        scoreboard.score += 1;
                    }

                    false
                }
                ((Some(_), _, _, _), (_, _, Some(_), _)) | ((_, _, Some(_), _), (Some(_), _, _, _)) => false,
                _ => false,
            };
            if resolve {
                match collision {
                    Collision::Top => {
                        left_phys.velocity.y = left_phys.velocity.y.abs();
                        right_phys.velocity.y = -right_phys.velocity.y.abs();
                    }
                    Collision::Bottom => {
                        left_phys.velocity.y = -left_phys.velocity.y.abs();
                        right_phys.velocity.y = right_phys.velocity.y.abs();
                    }
                    Collision::Left => {
                        left_phys.velocity.x = -left_phys.velocity.x.abs();
                        right_phys.velocity.x = right_phys.velocity.x.abs();
                    }
                    Collision::Right => {
                        left_phys.velocity.x = left_phys.velocity.x.abs();
                        right_phys.velocity.x = -right_phys.velocity.x.abs();
                    }
                }
            }
        }
    });
}

fn restart_key_system(
    commands: &mut Commands,
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<(Entity, Option<&mut PlayerShip>), Or<(With<Physics>, With<AsteroidSpawner>)>>,
    mut scoreboard_query: Query<&mut Scoreboard>
) {
    if keyboard_input.just_pressed(KeyCode::R) {
        for (entity, player) in query.iter_mut() {
            match player {
                None => {
                    commands.despawn(entity);
                }
                Some(mut player) => {
                    player.lives = 5;
                }
            }
        }
        for mut scoreboard in scoreboard_query.iter_mut() {
            scoreboard.score = 0;
        }
        commands.spawn((AsteroidSpawner { timer: ASTEROID_SPAWN_TIME * 3. - 0.5, .. Default::default() },));
    }
}

const BULLET_LIFE: f32 = 5.;

fn bullet_life_system(
    commands: &mut Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Bullet)>,
) {
    let delta = time.delta_seconds();
    for (entity, mut bullet) in query.iter_mut() {
        bullet.lifetime += delta;
        if bullet.lifetime >= BULLET_LIFE {
            commands.despawn(entity);
        }
    }
}