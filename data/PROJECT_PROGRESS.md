# 開発の引き継ぎ

## 目的

## 現在の状態

- 2026-11-20：野原/道/池/荒地をentity_templates/map/へ分離しcategory=map。現行map4/static11/movable18/item50の83定義。木・針葉樹はplacement=spawnとdestroyed_map（field/wasteland）、他staticはmap_gen。map.spawnsからの初期生成、通常spawn禁止/セル単位の下地置換API、Terrain Paletteで両分類参照、Import Map JSONとhouse/forest_housesサンプルを追加。PNGは既存static/パスを維持しmanifest/indexを更新。83+32枚の画像検査、本編246/Map Gen81テスト、Lua83定義と参照・既存模擬検査、両パッケージClippy/fmt/diffに成功。家シナリオは83定義/4543 Entity/83画像を読込し20ターン正常。関連資料とAGENTSを更新。実GUIと出力レイヤー分離・専用床素材は未確認/未追加。詳細と次の作業はルート進捗冒頭を参照。

- 2026-11-20：ゴブリンの漁り屋・鉄くず兵・槍持ち・弩撃ち・呼び子・頭目の6定義と専用32×32画像を追加。火薬屋はなし。現行83定義（movable18/item50/static15）・画像83枚・演出32枚。原画/生成指示/対応表は`entity_tex/source/goblins/`、画像だけの再処理は`--only-goblins`。既存画像109枚のハッシュ不変、全画像の寸法/指定32色/二値透過、Lua定義と参照を検証。既定テストマップを76×38へ拡張し東側13〜18室へ配置、集団戦用`scenarios/goblin_camp.lua`を追加。両マップの`--check-data`と拠点30ターンの正常動作を確認。PNG一覧は目視確認、実GUIは未確認。詳細・検証・次の作業はルート進捗と`document/goblins.md`を参照。

- 2026-11-20：`map_json/doukutu.json`の最終出力をNoise #1から、既存のTerrain MergeにつながるOutput #13へ修正。ノード・配線・地形設定は変更なし。本編`--map-json ... --check-data`で77定義・6,893 Entity・77画像の正常読込を確認。実GUIは未確認。詳細はルート進捗を参照。

- 2026-11-20：static画像15種を定義と照合。フォルダ外だった野原だけを`entity_templates/static.lua`から`entity_templates/static/field.lua`へ移動し、定義manifest・画像manifestの参照を更新。他14種に不足なし。全77 Lua定義と参照、77 Entity画像・32演出画像の検査成功。全109画像本体のハッシュは不変。Map Genの既定候補とstatic画像の全ID一致を検証。既存のマップ寸法テスト1件はJSON解像度64/期待256で失敗し、それを除く本編232テストは成功。実GUIは未確認。

- 2026-11-19：`scenarios/debug_map_json.lua`を追加。起動時に選んだ`map_json`のJSONへ既存の共通生成処理を使い、地形ID対応・Seed42・プレイヤー・空の追加配置はLuaで設定。`stone_wall`は既存石壁、`stone_floor`は既存野原へ対応（専用石床画像なし）。ユーザーの`shape_tiles.json`は変更なし。256×256・102,015 Entity・77定義/テクスチャの検査と本編全233テストが成功。実GUIは未確認。使い方・次の作業はルート進捗とREADMEを参照。

- 2026-11-18：ユーザーによるtoolへの配置復帰に合わせ、Map Gen関連5シナリオのJSON参照を`tool/map_gen`へ戻した。境界接続サンプルの設定は維持。検証結果はルート進捗を参照。

- 2026-11-18：Map Gen移動に合わせ既存4シナリオのJSONパスを`my_tool/map_gen`へ更新。地形別境界接続の`scenarios/stitched_tiles.lua`を追加し、`map_generator.tile = {0,0}`で共通タイル生成と外周壁の省略を利用する。77定義・1694 Entity・77テクスチャのデータ検査、本編全テストが成功。実GUI表示は未確認。詳細・次の作業はルート進捗を参照。

- 2026-11-16：体力・知力・社交を可動Entity12種から削除。4能力、レベルごとの最大スタミナ1〜5/最大正気度1〜10成長、制作dexterity_step=5、data/mental.luaの精神攻撃係数を追加。標準データの具体的な精神攻撃・回復操作は未実装。77定義の基本PNG・演出32枚は今回変更なし。詳細な動作・検証・次の作業はルート進捗を参照。

- 2026-11-16：消毒薬を定義・両マップ・有効PNGから削除。現行77定義（12/50/15）と基本画像77枚・演出32枚。元のアトラス内の絵は履歴のみ。source/atlas32-layout.jsonで元座標を固定し、manifest/index/見本をvalidate-onlyで更新した。

- 2026-11-16：弾丸8方向・命中火花4・回避残像4の専用32×32 RGBAを追加。演出は計32枚。原画はentity_tex/source/effects/bullet-sheet.png、登録はmanifest.effects。Entity78種・Luaの能力/威力は今回変更なし。

- 2026-11-16：手榴弾の飛翔・爆発・閃光の16フレームを専用32×32 RGBAで作成しmanifestのeffectsへ登録。原画・recipeはentity_tex/source/effects、見本はentity_tex/effects_preview.png。爆発手榴弾のLua威力を24→36（距離別36/24/12）へ変更。

- 2026-11-16：モジュール移動に合わせ `sound/README.md` の接続先を `src/media/sound.rs` へ更新。音源・生成方法・ゲームデータの変更なし。

## 決定事項

- 地面はmap分類で一セル一つ。初期生成の候補・確率はmap.spawns、staticはplacement=spawn/map_gen。spawnする破壊可能staticはdestroyed_mapにmap分類を指定する。子JSONはImportノードへ明示読込したスナップショットを保存する。

- 傷の汚染・消毒は採用しない。汎用状態削除APIは維持。パラメータの接続状況はdocument/mijisou.mdを参照。

- 近接の踏み込みは既存駒画像の位置補間で、専用差分PNGではない。ゲーム全体の表示順・確認結果はルート進捗を参照。

- 演出16枚はEntity78定義の基本画像とは別枠。32色・二値透過を維持し、--only-effectsで対象のみ再処理する。

- Rust構成と全体の検証結果はルートの `PROJECT_PROGRESS.md` と `document/class.pu` を参照する。

## 次の作業

- forest_houses.luaの実GUI、Terrain Paletteの両分類、Importノードの再読み込みと領域/マスク、破壊後の下地を確認する。コマンドと現状の制約はルート進捗冒頭を参照。

- 消毒を除いた棚の実GUI確認は未実施。

- 弾体・着弾画像と敵/プレイヤーの敏捷順のモーションを実GUIで確認。

- 飛翔・爆発・閃光と変更後威力の実GUI確認は未実施。

- 音声の実機試聴は引き続き未確認。

## 確認方法・結果

- 削除後Rust全216テスト・fmt・Clippy・既定/通常マップのデータ読込成功。Lua77定義・行動183・操作21・食料53の検査成功。素材77＋演出32枚を検査成功。

- 弾丸追加後、インポータで基本画像78枚・演出32枚を検査成功。原画・整形後の見本を目視確認。Rust全216テスト・fmt・Clippy・起動時のデータ検査成功。

- インポータで基本画像78枚・演出16枚を検査成功。原画と整形後の見本を目視確認。Rust全212テスト成功。

- 音声READMEの参照先ファイルの存在を確認。音源の再生成・試聴は今回実施していない。

