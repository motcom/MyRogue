//! Persistent chest contents and atomic, capacity-checked selection transfers.
use crate::{
    game::{Game, Panel},
    model::config::*,
    model::entity::*,
    ui::japanese::JapanesePrint,
};
use bracket_lib::prelude as bl;

impl Game {
    pub(crate) fn initial_container_contents(
        &self,
        item: &ItemDefinition,
        id: Id,
    ) -> Vec<ContainerEntry> {
        let Some(loot) = &item.loot else {
            return item.contents.clone();
        };
        let seed = map_gen::recipe::derive_seed(self.loot_seed, &id.to_string());
        let mut rng = crate::model::combat::Rng::new(seed);
        let mut pools = vec![Vec::new(); loot.tier_weights.len()];
        for entry in &loot.entries {
            let tier = self.entity_definitions[&entry.definition_id]
                .item
                .as_ref()
                .unwrap()
                .tier;
            pools[(tier - 1) as usize].push(entry);
        }
        let count = loot.min_rolls - 1 + rng.roll(loot.max_rolls - loot.min_rolls + 1);
        let mut contents = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let total = pools
                .iter()
                .zip(&loot.tier_weights)
                .filter(|(pool, _)| !pool.is_empty())
                .map(|(_, weight)| *weight)
                .sum();
            let mut roll = rng.roll(total);
            let (pool, _) = pools
                .iter_mut()
                .zip(&loot.tier_weights)
                .filter(|(pool, _)| !pool.is_empty())
                .find(|(_, weight)| {
                    if roll <= **weight {
                        true
                    } else {
                        roll -= **weight;
                        false
                    }
                })
                .expect("validated loot has enough weighted entries");
            let entry = pool.swap_remove((rng.roll(pool.len() as u32) - 1) as usize);
            let quantity =
                entry.min_quantity - 1 + rng.roll(entry.max_quantity - entry.min_quantity + 1);
            contents.push(ContainerEntry {
                definition_id: entry.definition_id.clone(),
                quantity,
            });
        }
        contents
    }

    pub(crate) fn open_container(&mut self, item: Id) -> Result<()> {
        ensure(
            self.can_reach_interaction(item),
            "その宝箱には手が届きません",
        )?;
        ensure(
            matches!(self.entities.get(&item), Some(Entity::Item(_)))
                && self
                    .definition(item)
                    .common
                    .tags
                    .iter()
                    .any(|tag| tag == "container"),
            "中身を開ける対象ではありません",
        )?;
        self.panel = Panel::Container { item };
        self.selection = 0;
        self.container_selection.clear();
        Ok(())
    }
    fn container_item(&self) -> Result<Id> {
        let Panel::Container { item } = self.panel else {
            return Err("宝箱を開いていません".into());
        };
        ensure(
            self.can_reach_interaction(item),
            "その宝箱には手が届きません",
        )?;
        ensure(
            self.entities[&item].script().state["opened"] == true,
            "宝箱は閉じています",
        )?;
        Ok(item)
    }
    pub(crate) fn container_entries(&self, item: Id) -> Vec<(usize, &ContainerEntry)> {
        let Some(Entity::Item(chest)) = self.entities.get(&item) else {
            return vec![];
        };
        chest
            .contents
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.quantity > 0)
            .collect()
    }
    pub(crate) fn select_container(&mut self, delta: i32) -> Result<()> {
        let item = self.container_item()?;
        let max = self.container_entries(item).len().saturating_sub(1);
        self.selection = (self.selection as i64 + i64::from(delta)).clamp(0, max as i64) as usize;
        Ok(())
    }
    pub(crate) fn toggle_container_selection(&mut self) -> Result<()> {
        let item = self.container_item()?;
        let index = self
            .container_entries(item)
            .get(self.selection)
            .map(|(index, _)| *index)
            .ok_or("宝箱は空です")?;
        if !self.container_selection.remove(&index) {
            self.container_selection.insert(index);
        }
        Ok(())
    }
    pub(crate) fn take_container_selection(&mut self) -> Result<()> {
        let item = self.container_item()?;
        ensure(
            !self.container_selection.is_empty(),
            "Spaceで取得する品を選んでください",
        )?;
        let actor = self.player_id;
        ensure(self.entities[&actor].alive(), "使用者が行動できません")?;
        let entries: Vec<_> = self
            .container_entries(item)
            .into_iter()
            .filter(|(index, _)| self.container_selection.contains(index))
            .map(|(index, entry)| (index, entry.clone()))
            .collect();
        ensure(
            entries.len() == self.container_selection.len(),
            "中身が変更されました",
        )?;
        let inventory = &self.actor(actor).ok_or("not actor")?.inventory;
        let mut mass = inventory
            .items
            .iter()
            .map(|id| self.item_mass(*id))
            .sum::<u64>();
        let mut volume: u64 = inventory
            .items
            .iter()
            .filter_map(|id| match &self.entities[id] {
                Entity::Item(i) => Some(u64::from(i.youryou) * u64::from(i.quantity)),
                _ => None,
            })
            .sum();
        for (_, entry) in &entries {
            let data = self.entity_definitions[&entry.definition_id]
                .item
                .as_ref()
                .unwrap();
            ensure(
                data.length_mm <= inventory.max_longsize,
                "取得する品が所持できる長さを超えています",
            )?;
            if self.entities[&item].carrier() != Some(actor) {
                mass += u64::from(data.mass_g) * u64::from(entry.quantity);
            }
            volume += u64::from(data.volume_ml) * u64::from(entry.quantity);
        }
        ensure(
            mass <= u64::from(inventory.max_jyuuryou) && volume <= u64::from(inventory.max_youryou),
            "選んだ品は重量または容量の上限を超えています",
        )?;
        let before = self.clone();
        let result = (|| -> Result<()> {
            for (index, entry) in entries {
                let Some(Entity::Item(chest)) = self.entities.get_mut(&item) else {
                    return Err("宝箱がありません".into());
                };
                chest.contents[index].quantity = 0;
                let limit = self.entity_definitions[&entry.definition_id]
                    .item
                    .as_ref()
                    .unwrap()
                    .stack_limit;
                let mut remaining = entry.quantity;
                while remaining > 0 {
                    let quantity = remaining.min(limit);
                    self.spawn(&entry.definition_id, None, Some(actor), quantity)?;
                    remaining -= quantity;
                }
                self.log(format!(
                    "{} x{} を取得しました",
                    self.entity_definitions[&entry.definition_id].common.name, entry.quantity
                ));
            }
            Ok(())
        })();
        if let Err(error) = result {
            *self = before;
            return Err(error);
        }
        self.container_selection.clear();
        self.selection = 0;
        self.advance_turn();
        Ok(())
    }
    pub(crate) fn render_container(&self, ctx: &mut bl::BTerm, opacity: f32) {
        let mut bg: bl::RGBA = bl::RGB::named(crate::ui::theme::BACKGROUND).into();
        bg.a = opacity;
        let white = bl::RGB::named(crate::ui::theme::TEXT);
        let yellow = bl::RGB::named(crate::ui::theme::ACCENT);
        for y in 5..=36 {
            for x in 3..=75 {
                ctx.set(x, y, white, bg, bl::to_cp437(' '));
            }
        }
        ctx.draw_hollow_box(3, 5, 72, 31, white, bg);
        ctx.print_ja_color_bg(5, 6, yellow, bg, "宝箱の中身");
        ctx.print_ja_color_bg(
            5,
            8,
            white,
            bg,
            "j/k・矢印:移動  Space:選択  Enter:取得  Esc:戻る",
        );
        let Panel::Container { item } = self.panel else {
            return;
        };
        let entries = self.container_entries(item);
        if entries.is_empty() {
            ctx.print_ja_color_bg(5, 11, white, bg, "宝箱は空です");
        }
        for (row, (cursor, (index, entry))) in entries
            .iter()
            .enumerate()
            .skip(self.selection.saturating_sub(19))
            .take(20)
            .enumerate()
        {
            let definition = &self.entity_definitions[&entry.definition_id];
            let tier = definition.item.as_ref().unwrap().tier;
            let label = format!(
                "{} [{}] {} x{}  Tier {}",
                if cursor == self.selection { ">" } else { " " },
                if self.container_selection.contains(index) {
                    "x"
                } else {
                    " "
                },
                definition.common.name.chars().take(48).collect::<String>(),
                entry.quantity,
                tier,
            );
            ctx.print_ja_color_bg(
                5,
                11 + row as i32,
                if cursor == self.selection {
                    yellow
                } else {
                    white
                },
                bg,
                label,
            );
        }
        ctx.print_ja_color_bg(
            5,
            34,
            white,
            bg,
            "選んだ品だけ取得します。残りは宝箱に残ります",
        );
    }
}

#[cfg(test)]
pub(super) fn set_test_chest_contents(game: &mut Game, chest: Id) {
    let Some(Entity::Item(item)) = game.entities.get_mut(&chest) else {
        panic!("expected a chest");
    };
    item.contents = [("item.bread", 2), ("item.bandage", 2), ("ammo.pistol", 10)]
        .into_iter()
        .map(|(definition_id, quantity)| ContainerEntry {
            definition_id: definition_id.into(),
            quantity,
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (Game, Id) {
        let mut game = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let remove: Vec<_> = game
            .entities
            .keys()
            .copied()
            .filter(|id| *id != game.player_id && game.entities[id].map_position().is_some())
            .collect();
        for id in remove {
            game.remove(id);
        }
        let origin = game.position(game.player_id).unwrap();
        let chest = game
            .spawn("item.treasure_chest", Some(origin.offset(1, 0)), None, 1)
            .unwrap();
        set_test_chest_contents(&mut game, chest);
        game.interact(chest, "open").unwrap();
        (game, chest)
    }
    #[test]
    fn generated_chests_roll_distinct_items_with_tier_weights_and_configured_quantities() {
        let mut game = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let position = game.position(game.player_id).unwrap();
        let mut same = game.clone();
        let mut combat_rng = game.rng.clone();
        let chest = game
            .spawn("item.treasure_chest", Some(position), None, 1)
            .unwrap();
        assert_eq!(
            chest,
            same.spawn("item.treasure_chest", Some(position), None, 1)
                .unwrap()
        );
        assert_eq!(
            game.entity_snapshot(chest).unwrap()["contents"],
            same.entity_snapshot(chest).unwrap()["contents"]
        );
        assert_eq!(game.rng.roll(1000), combat_rng.roll(1000));

        let item = game.definition(chest).item.as_ref().unwrap();
        let loot = item.loot.as_ref().unwrap();
        let mut first_draws = [0; 4];
        let mut sizes = std::collections::BTreeSet::new();
        for id in 1..=4096 {
            let contents = game.initial_container_contents(item, id);
            sizes.insert(contents.len());
            let mut ids = std::collections::BTreeSet::new();
            for entry in &contents {
                assert!(ids.insert(&entry.definition_id));
                let rule = loot
                    .entries
                    .iter()
                    .find(|rule| rule.definition_id == entry.definition_id)
                    .unwrap();
                assert!((rule.min_quantity..=rule.max_quantity).contains(&entry.quantity));
            }
            let tier = game.entity_definitions[&contents[0].definition_id]
                .item
                .as_ref()
                .unwrap()
                .tier;
            first_draws[(tier - 1) as usize] += 1;
        }
        assert_eq!(sizes, [2, 3, 4].into_iter().collect());
        assert!(first_draws.windows(2).all(|tiers| tiers[0] > tiers[1]));
        assert!((20..=70).contains(&first_draws[3]), "{first_draws:?}");
    }
    #[test]
    fn selection_toggle_cancel_and_capacity_failure_preserve_contents_and_time() {
        let (mut game, chest) = setup();
        let before = game.entity_snapshot(chest).unwrap();
        let count = game.entities.len();
        assert!(game.take_container_selection().is_err());
        game.toggle_container_selection().unwrap();
        game.toggle_container_selection().unwrap();
        assert!(game.container_selection.is_empty());
        game.toggle_container_selection().unwrap();
        game.select_container(1).unwrap();
        game.toggle_container_selection().unwrap();
        game.actor_mut(game.player_id)
            .unwrap()
            .inventory
            .max_youryou = 0;
        assert!(game.take_container_selection().is_err());
        assert_eq!(game.entity_snapshot(chest).unwrap(), before);
        assert_eq!(game.entities.len(), count);
        assert_eq!(game.turn, 1);
        assert_eq!(game.container_selection.len(), 2);
        game.close_panel();
        assert!(game.container_selection.is_empty());
        assert!(matches!(game.panel, Panel::World));
        game.interact(chest, "contents").unwrap();
        assert_eq!(game.turn, 1);
        assert_eq!(game.container_entries(chest).len(), 3);
    }
    #[test]
    fn unreachable_closed_or_deleted_chests_cannot_be_looted() {
        let (mut game, chest) = setup();
        game.toggle_container_selection().unwrap();
        let origin = game.position(game.player_id).unwrap();
        game.move_to(game.player_id, origin.offset(-1, 0)).unwrap();
        assert!(game.take_container_selection().is_err());
        assert_eq!(game.container_entries(chest).len(), 3);
        game.move_to(game.player_id, origin).unwrap();
        game.interact(chest, "close").unwrap();
        game.panel = Panel::Container { item: chest };
        game.container_selection.insert(0);
        assert!(game.take_container_selection().is_err());
        game.remove(chest);
        assert!(game.take_container_selection().is_err());
    }
    #[test]
    fn carried_contents_have_weight_and_transferring_preserves_total_mass() {
        let (mut game, chest) = setup();
        let player = game.player_id;
        let chest_mass = game.item_mass(chest);
        let base_mass = game.definition(chest).item.as_ref().unwrap().mass_g;
        assert!(chest_mass > u64::from(base_mass));
        game.actor_mut(player).unwrap().inventory.max_jyuuryou = base_mass;
        assert!(game.pickup_with_reach(player, chest, 1).is_err());
        game.actor_mut(player).unwrap().inventory.max_jyuuryou = 30000;
        game.pickup_with_reach(player, chest, 1).unwrap();
        let mass = |g: &Game| {
            g.actor(player)
                .unwrap()
                .inventory
                .items
                .iter()
                .map(|id| g.item_mass(*id))
                .sum::<u64>()
        };
        let before = mass(&game);
        game.actor_mut(player).unwrap().inventory.max_jyuuryou = before as u32;
        game.toggle_container_selection().unwrap();
        let mut cloned = game.clone();
        game.take_container_selection().unwrap();
        assert_eq!(mass(&game), before);
        assert_eq!(game.container_entries(chest).len(), 2);
        assert_eq!(cloned.container_entries(chest).len(), 3);
        cloned.select_container(1).unwrap();
        cloned.toggle_container_selection().unwrap();
        cloned.take_container_selection().unwrap();
        assert_eq!(cloned.container_entries(chest).len(), 1);
        assert_eq!(game.container_entries(chest).len(), 2);
    }
    #[test]
    fn failed_second_spawn_rolls_back_all_selected_transfers() {
        let (mut game, chest) = setup();
        game.toggle_container_selection().unwrap();
        game.select_container(1).unwrap();
        game.toggle_container_selection().unwrap();
        game.next_id = Id::MAX - 1;
        let snapshot = |g: &Game| {
            g.entities
                .keys()
                .map(|id| g.entity_snapshot(*id).unwrap())
                .collect::<Vec<_>>()
        };
        let before = snapshot(&game);
        assert!(game.take_container_selection().is_err());
        assert_eq!(snapshot(&game), before);
        assert_eq!(game.next_id, Id::MAX - 1);
        assert_eq!(game.turn, 1);
        assert_eq!(game.container_selection.len(), 2);
        assert_eq!(game.container_entries(chest).len(), 3);
    }
}
