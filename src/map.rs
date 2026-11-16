use crate::{
    config::*,
    entity::{Entity, EntityId},
};
use std::collections::BTreeMap;
#[derive(Clone, Default)]
pub struct Cell {
    pub entity_ids: Vec<EntityId>,
}
#[derive(Clone)]
pub struct Map {
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    pub cells: Vec<Cell>,
}
impl Map {
    pub fn new(width: i32, height: i32, depth: i32) -> Result<Self> {
        ensure(
            (5..=70).contains(&width) && (5..=38).contains(&height) && depth == 1,
            "initial map supports 5..70 x 5..38 x 1",
        )?;
        Ok(Self {
            width,
            height,
            depth,
            cells: vec![Cell::default(); (width * height * depth) as usize],
        })
    }
    pub fn contains(&self, p: Pos) -> bool {
        p.x >= 0
            && p.y >= 0
            && p.z >= 0
            && p.x < self.width
            && p.y < self.height
            && p.z < self.depth
    }
    fn index(&self, p: Pos) -> usize {
        ((p.z * self.height + p.y) * self.width + p.x) as usize
    }
    pub fn entities_at(&self, p: Pos) -> Vec<EntityId> {
        if self.contains(p) {
            self.cells[self.index(p)].entity_ids.clone()
        } else {
            vec![]
        }
    }
    pub fn insert(&mut self, p: Pos, id: EntityId) {
        let index = self.index(p);
        self.cells[index].entity_ids.push(id);
    }
    pub fn remove(&mut self, p: Pos, id: EntityId) {
        let index = self.index(p);
        self.cells[index].entity_ids.retain(|i| *i != id);
    }
    pub fn can_enter(
        &self,
        p: Pos,
        mover: EntityId,
        entities: &BTreeMap<EntityId, Entity>,
    ) -> bool {
        self.contains(p)
            && !self
                .entities_at(p)
                .iter()
                .any(|id| *id != mover && entities.get(id).is_some_and(Entity::blocks_movement))
    }
}
