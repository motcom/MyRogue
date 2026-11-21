-- ゴブリン6種と頭目の護衛をまとめて試す拠点。火薬屋は追加しない。
local spawns={}
local function put(id,x,y,n)
    spawns[#spawns+1]={definition_id=id,position={x=x,y=y,z=0},quantity=n or 1}
end
for _,entry in ipairs({{"scavenger",19,17},{"scrap_guard",24,12},{"spearman",25,13},
    {"crossbowman",28,10},{"caller",28,14},{"chief",32,12},{"scrap_guard",31,13}}) do
    put("monster.goblin_"..entry[1],entry[2],entry[3])
end
for x=22,35 do
    if x~=25 then put("obstacle.wooden_wall",x,7) end
    if x~=25 then put("obstacle.wooden_wall",x,19) end
end
for y=8,18 do put("obstacle.wooden_wall",35,y) end
put("item.treasure_chest",33,12)
put("item.bread",20,17,3);put("item.metal_stock",21,17,2)
put("item.crossbow",5,11);put("ammo.bolt",5,12,20)
put("item.bandage",5,13,5);put("item.shield",6,11);put("item.spear",6,12)
return {
    width=42,height=28,seed=42,ground="terrain.field",boundary="obstacle.stone_wall",
    player={definition_id="player.survivor",position={x=4,y=12,z=0}},
    spawns=spawns,labels={{text="ゴブリンの拠点",position={x=22,y=5,z=0}}},
}
