-- 家は別のJSON手続きのスナップショット。樹木とゴブリンは地面の初期スポーン表で配置する。
return {
    width = 64, height = 64, seed = 42,
    ground = "terrain.field", boundary = "obstacle.stone_wall",
    populate_map = true,
    map_generator = { json = "tool/map_gen/examples/forest_houses.json", inputs = {} },
    player = { definition_id = "player.survivor", position = { x = 2, y = 2, z = 0 } },
    spawns = {},
}
