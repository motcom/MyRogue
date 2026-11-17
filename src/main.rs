use bracket_lib::prelude as bl;
use bracket_lib::prelude::EMBED;
use my_rogue::media::sound::SoundPlayer;
use my_rogue::media::textures::TextureCatalog;
use my_rogue::{game::Game, model::config::Result};
use std::path::PathBuf;

struct PlayingGame {
    game: Game,
    audio: Option<SoundPlayer>,
}

impl bl::GameState for PlayingGame {
    fn tick(&mut self, ctx: &mut bl::BTerm) {
        self.game.tick(ctx);
        for sound in self.game.take_sounds() {
            if let Some(audio) = &mut self.audio {
                audio.play(sound);
            }
        }
    }
}
bl::embedded_resource!(JAPANESE_FONT, "assets/japanese16x16.png");
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut check = false;
    let mut simulate = None;
    let mut fullscreen = true;
    let mut mute = false;
    let mut ascii = false;
    let mut test_map = true;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = PathBuf::from(args.get(index).ok_or("--root needs a directory")?);
            }
            "--check-data" => check = true,
            "--test-map" => test_map = true,
            "--normal-map" => test_map = false,
            "--fullscreen" => fullscreen = true,
            "--windowed" => fullscreen = false,
            "--mute" => mute = true,
            "--ascii" => ascii = true,
            "--simulate" => {
                index += 1;
                simulate = Some(
                    args.get(index)
                        .ok_or("--simulate needs a turn count")?
                        .parse::<u32>()?,
                );
            }
            _ => return Err(format!("Unknown option: {}", args[index]).into()),
        }
        index += 1;
    }
    let mut game = if test_map {
        Game::load_test_map(&root)?
    } else {
        Game::load(&root)?
    };
    let textures = if ascii {
        None
    } else {
        Some(TextureCatalog::load(&root)?)
    };
    if check || simulate.is_some() {
        println!(
            "Loaded {} Lua definitions, {} Entity instances",
            game.entity_definitions.len(),
            game.entities.len()
        );
        if let Some(textures) = &textures {
            println!("Validated {} Entity textures (32x32 RGBA)", textures.len());
        }
        if let Some(turns) = simulate {
            for _ in 0..turns.min(10000) {
                if !game.entities.contains_key(&game.player_id) {
                    break;
                }
                game.advance_turn();
            }
            println!(
                "Simulated {} turns; player alive: {}",
                game.turn,
                game.entities.contains_key(&game.player_id)
            );
            for m in &game.messages {
                println!("{m}");
            }
        }
        return Ok(());
    }
    bl::link_resource!(JAPANESE_FONT, "resources/japanese16x16.png");
    let builder =
        bl::BTermBuilder::new()
            .with_dimensions(80, 50)
            .with_font("japanese16x16.png", 16, 16);
    let builder = if let Some(textures) = &textures {
        textures.embed()?;
        builder
            .with_font("entity-runtime.png", 32, 32)
            .with_sparse_console_no_bg(80, 50, "entity-runtime.png")
            .with_sparse_console_no_bg(80, 50, "entity-runtime.png")
    } else {
        builder
            .with_sparse_console(80, 50, "japanese16x16.png")
            .with_sparse_console_no_bg(80, 50, "japanese16x16.png")
    };
    let mut context = builder
        .with_sparse_console(80, 50, "japanese16x16.png")
        .with_simple_console(80, 50, "japanese16x16.png")
        .with_title("My Rogue - Lua Entities")
        .with_tile_dimensions(16, 16)
        .with_fullscreen(fullscreen)
        .with_fitscreen(true)
        // Keep the 80x50 layout and scale the whole console, including the UI.
        .with_automatic_console_resize(false)
        .build()
        .map_err(|e| e as Box<dyn std::error::Error>)?;
    if let Some(textures) = textures {
        textures.install(&mut game, 0);
    }
    game.set_map_consoles(0, 2);
    game.set_motion_console(1);
    context.set_active_console(3);
    let popup_console = context.register_console(bl::SimpleConsole::init(80, 50), 0);
    game.set_popup_console(popup_console);
    bl::INPUT.lock().activate_event_queue();
    let audio = if mute {
        None
    } else {
        match SoundPlayer::open(&root) {
            Ok(audio) => Some(audio),
            Err(error) => {
                eprintln!("Audio disabled: {error}");
                None
            }
        }
    };
    bl::main_loop(context, PlayingGame { game, audio })
        .map_err(|e| e as Box<dyn std::error::Error>)?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
