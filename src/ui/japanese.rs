//! 日本語の文字を、同梱したビットマップフォントのタイル番号へ変換する。
use bracket_lib::prelude::{BTerm, FontCharType, RGB, RGBA, to_cp437};
use std::{borrow::Cow, collections::HashMap, sync::OnceLock};

const CHARACTERS: &str = include_str!("../assets/japanese_chars.txt");

fn glyph(c: char) -> FontCharType {
    static GLYPHS: OnceLock<HashMap<char, FontCharType>> = OnceLock::new();
    if c.is_ascii() {
        return to_cp437(c);
    }
    *GLYPHS
        .get_or_init(|| {
            CHARACTERS
                .chars()
                .enumerate()
                .map(|(i, c)| (c, (256 + i) as FontCharType))
                .collect()
        })
        .get(&c)
        .unwrap_or(&to_cp437('?'))
}

pub trait JapanesePrint {
    fn print_ja(&mut self, x: i32, y: i32, text: impl AsRef<str>);
    fn print_ja_color(&mut self, x: i32, y: i32, fg: RGB, text: impl AsRef<str>);
    fn print_ja_color_bg(&mut self, x: i32, y: i32, fg: RGB, bg: RGBA, text: impl AsRef<str>);
}

impl JapanesePrint for BTerm {
    fn print_ja(&mut self, x: i32, y: i32, text: impl AsRef<str>) {
        self.print_ja_color(x, y, RGB::named(crate::ui::theme::TEXT), text);
    }

    fn print_ja_color(&mut self, x: i32, y: i32, fg: RGB, text: impl AsRef<str>) {
        self.print_ja_color_bg(
            x,
            y,
            fg,
            RGB::named(crate::ui::theme::BACKGROUND).into(),
            text,
        );
    }

    fn print_ja_color_bg(&mut self, x: i32, y: i32, fg: RGB, bg: RGBA, text: impl AsRef<str>) {
        let (width, height) = self.get_char_size();
        if y < 0 || y >= height as i32 {
            return;
        }
        for (offset, c) in text.as_ref().chars().enumerate() {
            let column = x + offset as i32;
            if column >= width as i32 {
                break;
            }
            if column >= 0 {
                self.set(column, y, fg, bg, glyph(c));
            }
        }
    }
}

/// 内部エラー文・テスト用ログは保持し、プレイヤー向け表示を翻訳する。
pub fn message(text: &str) -> Cow<'_, str> {
    let translated = match text {
        "Unsupported binding" => "このキー設定は未対応です",
        "No item here" => "足元にアイテムがありません",
        "No visible hostile target" => "見える範囲に敵がいません",
        "Unknown key" => "未対応のキーです",
        "blocked movement" => "そちらには進めません",
        "not a weapon" => "武器ではありません",
        "item not held" | "item not held/usable" => "そのアイテムは所持していません",
        "item is not here" => "足元にそのアイテムはありません",
        "inventory capacity exceeded" => "所持できる重さ・大きさ・容量を超えています",
        "this item has no consumable use" => "このアイテムは使用できません",
        "Used one item" => "アイテムを1個使用しました",
        "No effect; item was not consumed" => "効果がありません。アイテムは消費していません",
        "out of melee range" => "近接攻撃の範囲外です",
        "weapon unusable/out of range/ammo" => "武器を使用できません。射程・弾薬を確認してください",
        "Event limit reached; queue cleared" => "イベントの処理上限に達しました",
        _ => {
            if let Some((attacker, result)) = text.split_once(" -> ")
                && let Some((target, result)) = result.split_once(": ")
                && let Some((hit, result)) = result.split_once(" x")
                && let Some((count, damage)) = result.split_once(" damage ")
                && matches!(hit, "hit" | "miss")
            {
                return Cow::Owned(format!(
                    "{attacker} → {target}: {} 貫通{count}回 ダメージ{damage}",
                    if hit == "hit" { "命中" } else { "回避" }
                ));
            }
            return Cow::Borrowed(text);
        }
    };
    Cow::Borrowed(translated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japanese_ui_and_lua_characters_have_tiles() {
        // 生成時の収録対象にない文字をUIへ足したときも検出する。
        for source in [
            include_str!("mod.rs"),
            include_str!("input.rs"),
            include_str!("render.rs"),
            include_str!("../game/actions.rs"),
            include_str!("../game/progression.rs"),
            include_str!("../game/mental.rs"),
            include_str!("game_over.rs"),
            include_str!("../game/crafting.rs"),
            include_str!("footing.rs"),
            include_str!("japanese.rs"),
            include_str!("../game/mod.rs"),
            include_str!("../game/nutrition.rs"),
            include_str!("../game/player.rs"),
            include_str!("../game/world.rs"),
            include_str!("inspect.rs"),
            include_str!("../game/interactions.rs"),
            include_str!("../game/containers.rs"),
            include_str!("targeting.rs"),
            include_str!("properties.rs"),
            include_str!("../game/visibility.rs"),
            include_str!("animations.rs"),
            include_str!("melee.rs"),
        ] {
            for c in source.chars().filter(|c| !c.is_ascii()) {
                assert!(CHARACTERS.contains(c), "フォント未収録: {c}");
            }
        }
        let game = crate::game::Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("Lua definitions");
        for definition in game.entity_definitions.values() {
            for c in definition
                .common
                .name
                .chars()
                .chain(definition.common.description.chars())
                .filter(|c| !c.is_ascii())
            {
                assert!(CHARACTERS.contains(c), "フォント未収録: {c}");
            }
        }
        for recipe in &game.recipes {
            for c in recipe.name.chars().filter(|c| !c.is_ascii()) {
                assert!(CHARACTERS.contains(c), "unmapped recipe glyph: {c}");
            }
        }
        let png = include_bytes!("../assets/japanese16x16.png");
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!(width, 32 * 16);
        assert!(width / 16 * (height / 16) >= 256 + CHARACTERS.chars().count() as u32);
        assert_eq!(glyph('A'), to_cp437('A'));
        assert!(glyph('所') >= 256);
        assert_eq!(glyph('\u{10ffff}'), to_cp437('?'));
    }

    #[test]
    fn translates_combat_and_preserves_diagnostics() {
        assert_eq!(
            message("#1 -> #2: hit x2 damage 7"),
            "#1 → #2: 命中 貫通2回 ダメージ7"
        );
        assert_eq!(
            message("#1 -> #2: miss x0 damage 0"),
            "#1 → #2: 回避 貫通0回 ダメージ0"
        );
        assert_eq!(message("Lua #4: unknown error"), "Lua #4: unknown error");
    }
}
