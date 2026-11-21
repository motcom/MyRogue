# マップの基盤となるEntity

野原・道・荒地・池は `category = "map"` とし、定義を `data/entity_templates/map/` に置く。石壁・木壁・樹木・生垣・柵・家具・設備は `category = "static"` に残す。現在はmap4・static11・movable18・item50の計83定義。

map_entityは通常のスポーン対象にせず、Rustの `Game::set_map_entity` で各セルに一つ設定する。別の地面を設定すると以前の地面だけを置き換え、そのセルの固定物・アイテム・生物は維持する。`spawn` effectからmap分類を生成することは禁止する。既存シナリオの `ground` と `spawns` 内の地面指定は、読み込み時にこの設定APIへ渡す。

```lua
category = "map",
map = {
    movement_cost = 2.0,
    spawns = {
        { definition_id = "obstacle.tree", chance = 0.025 },
        { definition_id = "obstacle.conifer", chance = 0.015 },
    },
},
```

`common` の表示・衝突設定は他のEntityと共通で、地面の耐久は `false`。池は通行不可、野原・道・荒地は通行可能。PNGは従来の `data/entity_tex/static/terrain.*.png` を使用し、登録上のcategoryとLuaの参照先を更新している。PNGの再生成・縮小は行っていない。

`map.spawns` は初期生成時に何を置くかの候補。各候補の `chance` は有限数0〜1、`quantity` は省略時1。地面の定義ID、座標、seed、候補の並び順から決定的に判定する。一つ配置したセルには続けて配置しない。候補にmap分類は指定できず、staticは `placement = "spawn"` が必要。アイテム以外の数量は1、アイテムはstack_limitまでとする。野原では木（判定率2.5%）、針葉樹（1.5%）の順に試し、空いたセルでゴブリン6種を判定する。荒地は針葉樹0.5%、道・池は候補なし。

樹木の判定後に空いている野原1マスあたり、ゴブリン全体の目標出現率は2%。内訳は漁り屋50%、槍持ち22%、鉄くず兵15%、弩撃ち8%、呼び子4%、頭目1%。Luaの各 `chance` は、対象種の目標出現率を先行するゴブリン候補がすべて外れる確率で割っている。

スポーン候補に `with_item` でアイテム定義IDを指定すると、その候補が選ばれたときに同じマスへアイテムを1個置く。野原では頭目の候補に `with_item = "item.treasure_chest"` を指定している。シナリオの明示配置には既存の `spawns` を使う。

地面・Map Genの固定物・Luaの明示配置を先に読み込み、その後にスポーン表を処理する。通行不可の地面、固定物・アイテム・生物がすでにあるセル、開始位置から2マス以内には配置しない。戦闘用乱数とは独立し、通常ターンや破壊後の下地変更で再スポーンしない。

Luaシナリオの `populate_map = true/false` で初期スポーンを切り替える。省略時は `map_generator` 付きシナリオで有効。起動メニューと `--map-json` のJSON起動では、同じ `data/scenarios/debug_map_json.lua` で有効にする。JSON起動では生成地形から開始位置を決めた後、初期スポーンを行う。`forest_houses.lua` も有効。配置を固定する一部の生成サンプルでは明示的に無効にする。

staticは `static.placement = "map_gen"`（省略時の値）と `"spawn"` に分ける。石壁・生垣・柵・木壁・扉・家具・設備はmap_gen、木・針葉樹はspawn。これは初期生成の用途を示し、Map Genや固定テストマップでスポーン用定義を明示配置することもできる。

スポーンする破壊可能staticには `static.destroyed_map` を必須とする。参照先はmap分類の定義ID。仮設定は木→野原、針葉樹→荒地。破壊のLua hook（木材などのドロップ）を処理した後、Rustが下地を一度設定して破壊個体を削除する。map_gen分類の固定物を壊した場合は現在の下地を保つ。

家の生成とJSON取り込みは [Map Genの手順](../../tool/map_gen/README.md#import-map-json) を参照。既存の地形出力は1セルにつき1IDで、static IDのセルの下地はシナリオの `ground`。サンプルの室内床は暫定で道の画像を使い、家具の下などは既定の野原。家の専用床Entity・素材、地形と固定物を別々に保存する出力レイヤーは今回追加していない。
