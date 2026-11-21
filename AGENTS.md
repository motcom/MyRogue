# プロジェクトの作業ルール

- 作業前に README.md（存在する場合）と PROJECT_PROGRESS.md を読む。
- 作業の区切りに PROJECT_PROGRESS.md の現在の状態・決定事項・次の作業を更新する。
- 未確認の内容は、確認済みの内容と区別して記録する。

## ゲームの基本構造

- 基本構造は `document/basic_algorism.pu` に従う。
- ゲーム継続中は `input → update → render` の順で処理する。
- 入力・状態更新・描画は責務を分け、`GameState::tick` からこの順に呼び出す。
- ゲーム継続条件を満たさなくなったら終了する。
- Entityの定義・能力・行動と制作レシピ/調整ルールはLua、位置更新・消費・生成など整合性を守る共通処理はRustに置く。既存機能を組み合わせるEntity追加はLuaだけで完結させることを目標とし、新機能はRustの共通APIとして追加する。個別Entity IDによる分岐をRustへ増やさない。

## Entity画像とカラーパレット

- Entityテクスチャは `data/entity_tex` の32×32ピクセルRGBA PNGで管理する。16×16への縮小や単純な拡大で置き換えない。
- 可視ピクセルのRGBは `document/color_thema/palette.json`（Ash & Rust 32）の32色のみを使用する。GPLとの一致を維持し、独自色を追加しない。
- 地面（`map`）の可視ピクセルは明るさ60%超、移動と視界を両方遮る固定物（`static`）は50%未満。明るさは sRGB の各成分を `0.2126 R + 0.7152 G + 0.0722 B` で加重し、255を100%とする。配色の再処理には `python3 tools/import_entity_textures.py --only-recolor` を使う。
- 背景として反復する野原・道・池・荒地・石壁・木壁は視認性を優先し、基本画像は各3色程度・まとまりのある控えめな模様に抑える。細かなノイズ・砂利・強いハイライトを増やさない。
- アルファは0/255の二値。地形・壁6種以外は背景透過とし、最近傍補間・既存layer順で地形から重ねる。
- 素材変更時は `tools/import_entity_textures.py` で寸法・全定義との対応・32色制限・透過を検査し、manifestと一覧を更新する。実行時の読込でも寸法・パレット・アルファを検査する。

## テクスチャの作成状況と参照手順

- 作成状況はまずこの節で確認する。個別のEntity ID・名前・PNGパスの正本は `data/entity_tex/manifest.json` の `entries`。必要なIDだけ検索し、全Lua定義・全PNG・過去ログ・原画を毎回読み直さない。
- 2026-11-20時点の状況（今後追加・変更したらこの表も更新する）：

| 対象 | 状態 | 確認先・補足 |
| --- | --- | --- |
| 既存から消毒薬を除く63定義の専用基本画像 | 作成済み・ゲーム接続済み | movable 12、item 36、static 11、map 4。全て32×32 RGBA。IDとパスは `data/entity_tex/manifest.json` |
| 制作追加14定義の専用基本画像 | 作成済み・ゲーム接続済み | 工具7・材料6・制作ストレージ1。現行83定義（movable 18、item 50、static 11、map 4）は全て専用32×32 RGBA。追加分の原画・生成指示は `source/crafting/`、manifestの `source_override` と `dedicated_texture=true`。実GUIは未確認 |
| ゴブリン6種の専用基本画像 | 作成済み・ゲーム接続済み | 漁り屋・鉄くず兵・槍持ち・弩撃ち・呼び子・頭目。`source/goblins/recipe.json`と`prompts.json`、manifestの`source_override`。PNG一覧は目視確認済み、実GUIは未確認 |
| 地面4種・移動と視界を遮る固定物6種の明暗調整版 | 作成済み・ゲーム接続済み | 地面と壁は3色以内。配色は `source/palette_recolors.json`、背景6種の原画と生成指示は `source/simple_terrain/recipe.json`。PNGを目視確認済み、実GUIは未確認 |
| 道の方向別接続形状 | 実装済み（実行時生成） | `texture_rules.json` の接続グループと周囲8マスから派生。横/縦/曲がり角/分岐/広場に対応。専用PNGの追加生成は不要 |
| 同じ役割のランダム画像候補 | 登録・選択の仕組みを実装済み | 現在は各1候補。追加候補のPNGは未作成。`texture_rules.json` の `variants` に登録する |
| 扉・宝箱の閉状態 | 作成済み | 基本画像に含む |
| 扉・宝箱の開状態の専用画像 | 未作成 | ゲーム表示は基本画像の変形・印の重ね合わせによる代用。専用素材の作成済みとは数えない |
| 手榴弾・閃光弾の飛翔と爆発・閃光の演出画像 | 作成済み・ゲーム接続済み | 各4フレーム・計16枚の専用32×32 RGBA。manifestの `effects`、`source/effects/recipe.json`。Entity定義とは別枠。実GUIは未確認 |
| 弾丸8方向・命中火花4・回避残像4の演出画像 | 作成済み・ゲーム接続済み | 専用32×32 RGBAを16枚。manifestの `effects`、原画 `source/effects/bullet-sheet.png`。実GUIは未確認 |
| クロスボウボルトの飛翔8方向 | 作成済み・ゲーム接続済み | 専用32×32 RGBAを8枚追加し、演出は合計40枚。原画 `source/effects/bolt-sheet.png`。PNGは目視確認済み、実GUIは未確認 |
| プレイヤー・敵の近接攻撃モーション | 実装済み（表示位置の補間） | 既存の駒画像を0.32マス踏み込ませ約160msで戻す。専用差分PNGは追加していない。実GUIは未確認 |
| 死体・部位欠損・Entity本体の方向別・アニメーション差分画像 | 未作成 | 基本画像とは別の追加対象。投擲・爆発演出は上記の別枠 |
| 今後追加するEntity・上記に記載のない差分 | 未確認 | manifest未登録だけで未作成と断定せず、対象名・IDの範囲だけ確認する |

- 道の接続・柄違いの設定はまず `data/entity_tex/texture_rules.json` を対象IDだけ確認する。簡素版6種の再処理は `python3 tools/import_entity_textures.py --only-simple`。他のPNGを上書きしない。全体再処理でも簡素版の原画上書きを優先する。制作追加14種だけの再処理は `python3 tools/import_entity_textures.py --only-crafting`（既存63種を保持）。投擲・射撃演出40枚だけの再処理は `--only-effects`（対象以外のEntity PNGを保持）。登録・一覧の再検査だけなら `--validate-only` を使う。
- 見た目を確認する依頼のときだけ、対象PNGまたは `data/entity_tex/index.html` を参照する。`source/atlas32.png`・`source/prompt32.txt` は再生成や原画編集が必要なときだけ読む。旧 `source/atlas.png`・`source/prompt.txt` は16×16時代の履歴で、現行素材の確認には使わない。
- 素材の存在確認だけならmanifestの対象項目とそのPNGパスを調べる。生成スクリプトの再実行・画像生成・全素材検査は行わない。素材や定義の変更、不整合の発見、明示的な検証依頼がある場合は必要な検査を行う。
- 作成・差替え・削除したら、manifest（対象の登録情報）・この節（作成済み/未作成/未確認）・`PROJECT_PROGRESS.md` を同じ作業で更新する。実行時の代用表示、専用画像の有無、実GUI確認の有無は分けて記録する。

- 消毒薬は定義・配置・有効PNGから削除済み。旧64種の原画内の絵は履歴のみ。元アトラスの切出し位置は `source/atlas32-layout.json` で固定し、登録順の変更や削除で他の素材を取り違えない。
- 2026-11-20：static画像15種に対応するLua定義は全て `data/entity_templates/static/` に配置済み。野原は旧 `data/entity_templates/static.lua` から `static/field.lua` へ移動。Entity/PNGの追加はなく、Map Genの既定のstatic参照フォルダで全15種を取得できる。

- 2026-11-20：ゴブリン6定義・専用画像を追加し現行83定義（movable 18、item 50、static 11、map 4）。ゴブリンだけの画像再処理は`python3 tools/import_entity_textures.py --only-goblins`。他の77画像と全演出画像を保持する。

- 2026-11-20：野原・道・池・荒地の4定義を`data/entity_templates/map/`へ分離しcategory=mapへ変更。残るstaticは11定義。PNGは従来のstatic/配下を保持し、manifestの分類とLua参照を更新。Map Genの既定参照ルートはentity_templates全体でmap/static双方を候補にする。過去の「static15種」は分離前の記録。
- map_entityは通常spawnしない。地面は`set_map_entity`でセルごとに一つ設定し、Luaのmap.spawnsを初期生成で処理する。static.placementはmap_gen/spawnに分け、spawnする破壊可能staticにはmap分類を指すdestroyed_mapを指定する。木→野原、針葉樹→荒地は仮設定。詳細はdocument/entity_document/map_entity.md。
