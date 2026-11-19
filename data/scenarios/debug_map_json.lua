-- 起動時のJSON選択に使うデバッグ設定。地形IDの対応・Seed・プレイヤーはLuaで管理。
-- Terrain PaletteにはEntityの正式IDも直接指定できる。
return {
    width = 32, height = 32, seed = 42,
    ground = "terrain.field", boundary = "obstacle.stone_wall",
    map_generator = {
        json = "", -- 起動時に選択したJSONパスを設定する。
        ids = {
            stone_wall = "obstacle.stone_wall",
            stone_floor = "terrain.field",
        },
    },
    player = { definition_id = "player.survivor", position = { x = 16, y = 16, z = 0 } },
    spawns = {},
}
