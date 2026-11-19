-- このJSONをMap Genで再編集すると、次回のゲーム起動にも反映される。
return {
    width = 32, height = 32, seed = 42,
    ground = "terrain.field", boundary = "obstacle.stone_wall",
    map_generator = {
        json = "tool/map_gen/examples/game_map.json",
        inputs = {},
        terrain = {
            { max = 0.5, definition_id = "obstacle.stone_wall" },
            { max = 1.0, definition_id = "terrain.road" },
        },
    },
    player = { definition_id = "player.survivor", position = { x = 16, y = 16, z = 0 } },
    spawns = {},
}
