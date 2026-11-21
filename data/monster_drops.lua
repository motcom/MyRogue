-- モンスターのティアと同じ枠から1品目を選ぶ。枠内の品目は等確率。
return {
    [1] = {
        { definition_id = "item.bandage", min_quantity = 1, max_quantity = 2 },
        { definition_id = "item.bread" },
        { definition_id = "item.clean_water" },
        { definition_id = "item.cloth", min_quantity = 1, max_quantity = 3 },
        { definition_id = "item.wood", min_quantity = 1, max_quantity = 2 },
        { definition_id = "ammo.bolt", min_quantity = 2, max_quantity = 4 },
    },
    [2] = {
        { definition_id = "ammo.pistol", min_quantity = 3, max_quantity = 7 },
        { definition_id = "item.vitamins" },
        { definition_id = "item.short_sword" },
        { definition_id = "item.leather_armor" },
        { definition_id = "item.shield" },
        { definition_id = "item.wood_saw" },
    },
    [3] = {
        { definition_id = "ammo.rifle", min_quantity = 3, max_quantity = 7 },
        { definition_id = "item.syringe" },
        { definition_id = "item.flash_grenade" },
        { definition_id = "item.long_sword" },
        { definition_id = "item.forging_hammer" },
    },
    [4] = {
        { definition_id = "item.frag_grenade" },
        { definition_id = "item.ak" },
        { definition_id = "item.m16" },
    },
}
