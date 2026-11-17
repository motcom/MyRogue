//! モジュールの正規配置は model / game / ui / media / scripting。
//! ルートの再公開は既存の利用側・統合テスト向けの互換パス。

pub mod game;
pub mod media;
pub mod model;
pub mod scripting;
pub mod ui;

// 既存の公開 API を保持する。新しいコードは上記の正規パスを使う。
pub use game::{
    crafting, enemies, equipment, interactions, player, progression, statistics, treatment,
    visibility, world,
};
pub use media::{sound, textures};
pub use model::{combat, config, entity, map};
pub use ui::{inspect, japanese, projectiles, properties, quick_slots, targeting, theme};
