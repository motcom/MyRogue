-- このJSONをMap Genで再編集すると、次回のゲーム起動にも反映される。
return {
    width = 32, height = 32, seed = 42,
    ground = "terrain.field", boundary = "obstacle.stone_wall",
    map_generator = {
        json = "tool/map_gen/examples/game_map_ids.json",
        inputs = {},
    },
    player = { definition_id = "player.survivor", position = { x = 16, y = 16, z = 0 } },
    spawns = {},
}
