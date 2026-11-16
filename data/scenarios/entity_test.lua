-- 全定義を試す専用マップ。通常のscenario.luaとは独立。
local spawns, labels = {}, {}
local function put(id, x, y, quantity)
    table.insert(spawns, {definition_id=id, position={x=x,y=y,z=0}, quantity=quantity or 1})
end
local function label(text, x, y)
    table.insert(labels, {text=text, position={x=x,y=y,z=0}})
end
local function row(ids, y, spacing, quantity)
    for i,id in ipairs(ids) do put(id,3+(i-1)*spacing,y,quantity) end
end
label("近接武器",3,2)
row({"item.knife","item.club","item.short_sword","item.long_sword",
     "item.bastard_sword","item.mace","item.spear","item.cleaver"},3,3)
label("射撃武器",3,5)
row({"item.glock","item.crossbow","item.sks","item.ak","item.m16"},6,4)
label("弾薬 / 投擲",3,8)
put("ammo.pistol",3,9,50); put("ammo.bolt",7,9,20); put("ammo.rifle",11,9,50)
put("item.frag_grenade",15,9,5); put("item.flash_grenade",19,9,5)
label("回復 / 栄養剤",3,10)
row({"item.bandage","item.syringe","item.vitamins","item.disinfectant"},11,4,3)
label("食料 / 飲料",3,13)
row({"item.apple","item.raw_meat","item.cooked_meat","item.rotten_meat","item.bread",
     "item.carrot","item.cabbage","item.mineral_water","item.dirty_water","item.clean_water"},14,2,3)
label("製作 / 装備 / 宝箱",3,16)
put("item.crafting_storage",5,17)
row({"item.hand_drill","item.metal_saw","item.metal_file","item.forging_hammer","item.sewing_kit","item.cooking_kit","item.wood_saw",
     "item.metal_stock","item.cloth","item.leather_piece","item.thread","item.charcoal","item.wood_handle"},17,1)
put("facility.workbench",4,18); put("item.wood",5,18,5); put("item.hammer",7,18)
put("item.leather_armor",11,18); put("item.shield",15,18); put("item.treasure_chest",23,18)
label("固定物 / 遮蔽",3,23)
row({"obstacle.tree","obstacle.conifer","obstacle.hedge","obstacle.fence",
     "obstacle.wooden_wall","obstacle.chair","obstacle.table","obstacle.lamp","terrain.pond"},21,3)
label("野原2 / 荒地4 / 道1",3,26)
for x=3,7 do for y=27,29 do put("terrain.field",x,y) end end
for x=11,15 do for y=27,29 do put("terrain.wasteland",x,y) end end
-- Each room is sealed. Open only the door of the enemy being tested.
local enemies = {
    {"monster.ghoul_worker","01通常"}, {"monster.ghoul_warrior","02戦士"},
    {"monster.ghoul_archer","03弓兵"}, {"monster.ghoul_runner","04走者"},
    {"monster.ghoul_butcher","05鉈"}, {"monster.ghoul_guard","06盾"},
    {"monster.ghoul_howler","07叫び"}, {"monster.ghoul_scavenger","08拾得"},
    {"monster.ghoul_stalker","09待伏"}, {"monster.ghoul_brute","10巨体"},
    {"monster.ghoul_rifleman","11銃兵"},
}
for i=1,12 do
    local x0=30+((i-1)%4)*7
    local y0=2+math.floor((i-1)/4)*10
    for x=x0,x0+6 do for y=y0,y0+6 do
        if x==x0 or x==x0+6 or y==y0 or y==y0+6 then
            put((x==x0+3 and y==y0+6) and "obstacle.wooden_door" or "obstacle.stone_wall",x,y)
        end
    end end
    local e=enemies[i]
    label(e and e[2] or "12爆発",x0,y0+7)
    if e then put(e[1],x0+3,y0+3) end
end
-- Scavenger has a gun and ammunition to discover and equip inside its room.
put("item.glock",54,15); put("ammo.pistol",55,15,20)
-- Blast chamber: three destructible targets at distances 0/1/2 and a shielding wall.
put("obstacle.chair",54,25); put("obstacle.chair",55,25); put("obstacle.chair",56,25)
put("obstacle.wooden_wall",55,24)
-- A separate stone warehouse protects preparation even when enemy rooms are open.
for x=1,27 do for y=1,20 do
    if x==1 or x==27 or y==1 or y==20 then
        put((x==27 and y==19) and "obstacle.wooden_door" or "obstacle.stone_wall",x,y)
    end
end end
label("倉庫",18,2)
label("出口",25,19)
return {
    width=60, height=38, seed=42,
    ground="terrain.road", boundary="obstacle.stone_wall",
    player={definition_id="player.survivor",position={x=4,y=4,z=0}},
    spawns=spawns, labels=labels,
}
