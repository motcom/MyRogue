-- tileはワールド上のチャンク座標。同じseedとJSONで隣も再現できる。
return {
    populate_map = false,
    width = 32, height = 32, seed = 42,
    ground = "terrain.field", boundary = "obstacle.stone_wall",
    map_generator = {
        json = "tool/map_gen/examples/stitched_tiles.json",
        tile = { 0, 0 },
        inputs = {},
    },
    player = { definition_id = "player.survivor", position = { x = 16, y = 16, z = 0 } },
    spawns = {},
}
