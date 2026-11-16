-- 最初の動作確認用マップ。全ての地面・固定物・生物・品物を Entity として生成。
local spawns = {
    { definition_id = "monster.ghoul_worker", position = {x=20,y=10,z=0} },
    { definition_id = "monster.ghoul_warrior", position = {x=28,y=16,z=0} },
    { definition_id = "monster.ghoul_archer", position = {x=30,y=8,z=0} },
    { definition_id = "terrain.pond", position = {x=16,y=17,z=0} },
    { definition_id = "facility.workbench", position = {x=7,y=9,z=0} },
    { definition_id = "item.glock", position = {x=9,y=10,z=0} },
    { definition_id = "ammo.pistol", quantity=20, position = {x=10,y=10,z=0} },
    { definition_id = "item.sks", position = {x=11,y=10,z=0} },
    { definition_id = "ammo.rifle", quantity=20, position = {x=12,y=10,z=0} },
    { definition_id = "item.disinfectant", position = {x=8,y=11,z=0} },
    { definition_id = "item.syringe", position = {x=9,y=11,z=0} },
    { definition_id = "item.leather_armor", position = {x=10,y=11,z=0} },
}
for x=3,35 do table.insert(spawns, {definition_id="terrain.road",position={x=x,y=12,z=0}}) end
for y=5,8 do table.insert(spawns, {definition_id="obstacle.stone_wall",position={x=15,y=y,z=0}}) end
for x=17,19 do table.insert(spawns, {definition_id="obstacle.tree",position={x=x,y=15,z=0}}) end
-- 初期位置の西側に、扉・床・宝箱のある小さな建物を配置する。
for y=5,9 do for x=2,6 do
    if x==2 or x==6 or y==5 or y==9 then
        table.insert(spawns, {definition_id=(x==6 and y==8) and "obstacle.wooden_door" or "obstacle.stone_wall",position={x=x,y=y,z=0}})
    else
        table.insert(spawns, {definition_id="terrain.road",position={x=x,y=y,z=0}})
    end
end end
for x=7,8 do table.insert(spawns, {definition_id="terrain.road",position={x=x,y=8,z=0}}) end
for _, p in ipairs({{x=4,y=7,z=0}, {x=9,y=9,z=0}}) do
    table.insert(spawns, {definition_id="item.treasure_chest",position=p})
end
local additions = {
    "monster.ghoul_runner",
    "monster.ghoul_butcher",
    "monster.ghoul_guard",
    "monster.ghoul_howler",
    "monster.ghoul_scavenger",
    "monster.ghoul_stalker",
    "monster.ghoul_brute",
    "monster.ghoul_rifleman",
}
for i, id in ipairs(additions) do table.insert(spawns, {definition_id=id,position={x=39+(i%4)*4,y=18+math.floor((i-1)/4)*5,z=0}}) end
local supplies = {
    "item.short_sword",
    "item.long_sword",
    "item.bastard_sword",
    "item.mace",
    "item.spear",
    "item.cleaver",
    "item.ak",
    "item.m16",
    "item.apple",
    "item.raw_meat",
    "item.cooked_meat",
    "item.rotten_meat",
    "item.carrot",
    "item.cabbage",
    "item.dirty_water",
    "item.clean_water",
    "item.vitamins",
    "item.frag_grenade",
    "item.flash_grenade",
    "terrain.wasteland",
    "obstacle.table",
    "obstacle.chair",
    "obstacle.wooden_wall",
    "obstacle.conifer",
    "obstacle.hedge",
    "obstacle.fence",
    "obstacle.lamp",
    "item.shield",
}
for i, id in ipairs(supplies) do table.insert(spawns, {definition_id=id,position={x=22+((i-1)%10)*2,y=23+math.floor((i-1)/10)*2,z=0}}) end
return {
    width=60, height=38, seed=42,
    ground="terrain.field", boundary="obstacle.stone_wall",
    player={definition_id="player.survivor",position={x=8,y=10,z=0}},
    spawns=spawns,
}
