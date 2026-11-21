# ゴブリン

荒廃した世界の廃材や拾った装備で生き延びる6種。火薬屋は含まない。数値はゲーム調整用の初期値。

| 種類 | Entity ID | HP / 基礎AV | 装備・行動 |
| --- | --- | --- | --- |
| 漁り屋 | `monster.goblin_scavenger` | 16 / 0 | ナイフ。近くの物資を拾い、拾った武器は次のターンに装備。HPが1/3以下なら敵から後退 |
| 鉄くず兵 | `monster.goblin_scrap_guard` | 32 / 2 | 棍棒と盾。偶数ターンだけ行動する鈍足の前衛。盾の防御は基礎AVへ別途加算 |
| 槍持ち | `monster.goblin_spearman` | 20 / 0 | 槍を両手持ち。既存の射程2と遮蔽・腕の検査で攻撃 |
| 弩撃ち | `monster.goblin_crossbowman` | 18 / 0 | クロスボウ、ボルト3本、予備ナイフ。矢がある間は距離2以内で後退し、撃ち切ったら次の行動で持ち替えて近接戦に移る |
| 呼び子 | `monster.goblin_caller` | 16 / 0 | 缶の笛。敵を視認すると半径10の同族へ敵の位置を通知。有効6ターン、7ターンごとに再使用。接近されたら後退 |
| 頭目 | `monster.goblin_chief` | 44 / 1 | ショートソード・盾・革の鎧。前衛として戦う。拠点シナリオには鉄くず兵の護衛を配置 |

命中補正は弩撃ちが−2、ほかの5種が−1。鉄くず兵の筋力は7、頭目の筋力は8。

全種の派閥は`goblin`。Luaの`hostile_factions`で生存者・グールと敵対し、同族は敵として選ばない。敵がいなければ周囲を徘徊し、警報中は最後に通知された位置へ向かう。鉄くず兵の行動間隔は警報追跡時にも維持する。

野原で頭目が初期スポーンすると、そのマスに宝箱を1個置く。配置ルールは野原の `map.spawns` にある。

定義・判断・調整は `data/entity_templates/movable/goblin_*.lua`。弩撃ちのLuaは装填済みの矢と装填可否を参照し、必要なら所持品からナイフを選んで装備アクションを返す。Rustは実際の弾薬消費、持ち替え、射程や移動の検査を共通処理として担う。

## 試す

既定のEntityテストマップは76×38へ拡張し、既存12室の位置を維持して東側に13〜18室を追加した。13室のパンは拾得試験用。

```sh
cargo run -- --test-map --windowed
cargo run -- --scenario data/scenarios/goblin_camp.lua --windowed
```

拠点には6種と頭目の護衛1体、宝箱・物資を配置。開始地点の武器と回復品で準備してから東へ進む。`doukutu.json` などのJSONから起動した生成マップでは、野原の `map.spawns` に沿ってゴブリンを初期配置する。

## 画像

専用の32×32 RGBA画像6枚を`data/entity_tex/movable/monster.goblin_*.png`に登録。Ash & Rust 32色・二値アルファ・背景透過。原画は内蔵imagegenで個別生成し、16×16を経由せず既存の取り込み処理で32×32へ変換した。

- [6種の画像一覧](../data/entity_tex/goblins_preview.png)
- [生成指示](../data/entity_tex/source/goblins/prompts.json)
- [原画との対応](../data/entity_tex/source/goblins/recipe.json)

再取り込みは`python3 tools/import_entity_textures.py --only-goblins`。他の77 Entity画像と全演出画像を保持する。専用の歩行差分・方向別差分・死体画像は追加していない。基本画像一覧の目視とデータ検査は確認済み、実GUIは未確認。
