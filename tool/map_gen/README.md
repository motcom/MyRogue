# Map Gen — Node Material Lab

Substance Designerのように画像処理ノードを接続して、ノイズ・マスク・高さマップを作る独立したRustアプリの初期版。
生成にbracket-noise、編集画面にeframe/eguiを使用する。GUI非依存のライブラリを本編と共有する。
現段階は**グレースケール画像のノード編集と地形プレビュー**を対象とする。

## 起動

本編のルートから：

```sh
cargo run --manifest-path tool/map_gen/Cargo.toml
```

`tool`はユーザー指定の単数形。初回は検証用パッケージと依存ライブラリをビルドする。
独立したCargo.lockで、bracket-noiseは本編と同じコミット`ce9124ad744553b2576e44df44e94b0f72faa8e8`に固定している。
本編のバイナリ・ライブラリをコンパイルする必要はない。

最初から次のグラフが接続されている。

```text
Noise (Simplex) ── Background ──┐
                               Blend (Overlay) → Levels → Output
Noise (Perlin) ── Foreground ──┘
```

## 画面と操作

| 場所・操作 | 機能 |
| --- | --- |
| 左のNODE LIBRARY | 役割別の折りたたみ一覧。検索でノード名・役割名を絞り込む。追加位置は表示中のGRAPH中央 |
| 中央のGRAPH | 各ノードの正方形のサムネイルと接続を表示 |
| ノードをクリック | 選択し、右のPROPERTIESでパラメータ編集 |
| ノードの端子以外をドラッグ | 画像部分を含むどこでもノード移動。配置だけの変更ではノイズを再計算しない |
| ノードをダブルクリック | 下部の大きいプレビューへ表示を固定。別ノードの単クリックでは切り替わらない |
| 出力端子から入力端子へドラッグ | 接続。端子を順にクリックしてもよい |
| 接続線をクリック | 画像入力・数値入力の接続線を選択して強調表示。ノード選択と切り替わる |
| 入力端子を右クリック | その接続を解除。右側のINPUTSの×でも解除可能 |
| グラフ背景を左ドラッグ | 矩形が少しでも触れるノードを複数選択。背景クリックで解除 |
| 選択済みノードをドラッグ | 選択した全ノードを相対位置・配線を維持して移動。Undo/Redoに対応 |
| グラフ背景を中ボタン、またはAlt（Option）＋左ドラッグ | パン |
| GRAPH上でトラックパッドの2本指スクロール | 縦横にパン。ノード上でも操作可能、OSの慣性スクロールにも対応 |
| GRAPH上でピンチ | 開くと拡大、閉じると縮小。カーソル位置を中心にズーム |
| GRAPH上でマウスホイール | カーソル位置を基準にズーム（スクロール量が正なら拡大、負なら縮小） |
| グラフ背景を右クリック | ノード追加メニュー。追加位置は現在見えているGRAPH領域の中央 |
| 100% | 縮小したノードを等倍へ戻す。起動時も100%。ノード名18、入力名14ポイント（ズームに追従） |
| 全体表示 | 全ノードが入るように表示位置・倍率を調整 |
| Delete / Backspace | テキスト入力中でなければ選択した接続線を解除、または選択した全ノードを削除 |
| Esc | 接続途中の線をキャンセル |
| Undo / Redo | パラメータ・接続・追加/削除・配置の変更を戻す/やり直す。連続したドラッグ操作はまとめる。最大100履歴 |
| Cmd+Z / Cmd+Shift+Z | テキスト入力中でなければUndo / Redo。Windows/LinuxはCtrlを使用 |
| 下部の2D VIEW | ダブルクリックしたノードの途中結果、または「最終出力」ボタンで出力を表示。初期高さは最大760（上のグラフ用に220を確保し、画面高さに合わせて調整）。画像全体をパネル内に収まる大きな正方形で表示する。境界をドラッグして高さを変更できる |
| Terrain Paletteノード | 入力値の範囲ごとに地形IDとRGB色を設定。サムネイル・プレビュー・PNGへ反映し、設定をJSONへ保存 |
| プレビュー上のカーソル | セル座標と0〜1の値を表示 |
| このノードを最終出力にする | 任意のノードをPNG書出し対象にする |

トラックパッドのクリックドラッグでも矩形選択・ノード移動・配線ができる。中ボタンの代わりに2本指スクロールでパンでき、Alt（Option）＋クリックドラッグも引き続き使える。
ライブラリ・プロパティ・JSONファイルブラウザもマウスホイールだけ逆方向。2本指スクロール・慣性・ドラッグ操作は元の方向を維持する。
マウス/トラックパッドの種別は入力イベントから判別し、種別不明の環境ではPoint単位をパン、Line/Page単位をホイールズームとして扱う。

左の「マップを保存 / 開く」に保存・再読込をまとめている。
**保存するのはマップJSONひとつ。同じファイルを再編集にもゲームの生成にも使う。**

1. 「保存先を選ぶ…」でフォルダ・ファイル名を選んで保存するか、パスを入力して「JSONを保存」。
2. 再編集するときは「JSONを開く…」から同じJSONを選ぶ。
3. ゲームではLuaシナリオの`map_generator.json`にそのJSONのパスを指定する。

JSONには全ノード・名前・画面上の位置・接続・数値入力・解像度・最終出力を保存する。
未使用の枝や編集中の未接続ノードも残る。ゲーム実行時は最終出力に必要な枝だけ評価する。
最終出力に必要な入力が未接続なら、保存はできても生成時にエラーになる。
旧Graph JSONはそのまま利用でき、旧`recipe_version=1`も読み込める。
旧レシピに含まれない位置・名前・未使用の枝は復元できず、自動配置になる。

PNGが必要な場合だけ、右側の「画像を書き出す（任意）」から出力する。JSONを追加出力しない。
既定の保存先はこのフォルダの`output/map.json`。保存は同名ファイルを上書きする。
地形の色とIDはTerrain Paletteノードとして保存する。パン・ズーム、選択ノード、Undo履歴、試行用の実行Seedは保存しない。
生成Seedと外部入力は実行側から渡す。

左の一覧は「数値入力」「形状・ノイズ」「配置・通路」「壁・入口」「領域処理」「画像加工」「地形ID・出力」の7群。各群を折りたため、検索中は一致する群を展開する。

## 実装済みノード

| ノード | 入力 | パラメータ・処理 |
| --- | --- | --- |
| Number Input | なし（数値出力） | 定数 / Min・Max＋ランダム指定、仮の値/既定値、整数、外部入力名 |
| Noise | なし | Simplex / Perlin / Value / Cellular / White、seed、周波数。前3種はFBMのoctaves・gain・lacunarity |
| Shape | なし | Circle / Square / Triangle / Diamond / Hexagon、Scale X/Y、回転、内側へぼかすSoftness |
| Connect Regions | Source | 白いブロックを全体接続。太さ1〜64px、歪み、うねりの頻度、領域判定しきい値、seed、元画像も出力するか |
| Tile Sampler | Source | X/Y個数1〜64、全体スケール・Scale X/Y、サイズ・位置X/Y・回転のランダム量、seed |
| Uniform | なし | 0〜1の一定値 |
| Blend | Background、Foreground、任意のMask | 通常・乗算・スクリーン・オーバーレイ・加算・減算・最小・最大、不透明度 |
| Levels | Source | コントラスト・明るさ・ガンマ |
| Invert | Source | `1-v`で反転 |
| Black to Alpha | Source | 完全な黒（出力RGB 0,0,0）だけを透明化。既存の透明度と地形IDを維持 |
| Threshold | Source | 境界値、境界のぼかし幅。幅0なら二値化 |
| Blur | Source | ピクセル単位の半径0〜32。縦横に分離したボックスフィルター |
| Directional Warp | Source、Displacement | 変位画像の0.5を基準に斜め45度方向へ変位。強さ0〜0.25、双線形補間 |
| Morphology | Source | 円形の距離で白い領域を拡大/縮小。半径0〜64px |
| Wall Outline | Source | 白い領域の外側に幅1〜64pxの壁マスク |
| Distance Map | Source | 壁までの内側距離／白い領域までの距離を0〜1で出力 |
| Mask Scatter | Source | マスク内に最小間隔を空けて点・円形マーカーを配置。個数・半径・seed |
| Entrance Candidates | Rooms、Corridors | 入口マスク。正方形/矩形・幅/奥行き・外側オフセット・壁方向への自動回転・対向候補の統合 |
| Extra Connections | Rooms、Existing Paths | 基本の木構造以外の通路を追加。追加数・太さ・歪み・seed |
| Select Regions | Source | 面積のMin/Maxで選別、または条件に合う最大領域だけを残す |
| Cellular Automata | Source | 8近傍の白い数による誕生/生存ルールを反復。洞窟などの形状作り |
| Partition | Source | 領域を矩形で再帰分割。最小の子区画寸法・深さ・仕切り幅・seed |
| Reachable Area | Source | 指定した開始座標から4近傍で到達可能な白い領域のみを残す |
| Terrain Merge | Background、Foreground、任意のMask | 複数パレットのID・色・透明度を保って合成。前景優先・透明は背景、マスクで選択 |
| Terrain Palette | Source | 範囲の上限・地形ID・表示RGB色を最大32件指定。ゲームも同じID分類を使用 |
| Output | Source | 入力画像または地形ID付き画像を最終出力へ渡す |

Blendのマスク未接続は白（1）として扱う。黒で効果なし、白で不透明度どおりの効果。
未接続の必須入力があるノードはエラーにし、別の独立した正常ノードは引き続き評価する。
循環する配線は拒否し、元の接続を維持する。ノード削除時には関連する入力を切り離す。

## 数値入力と実行Seed

NODE LIBRARYから`Number Input`を追加する。入力名は英数字・アンダースコアで最大64バイト、グラフ内で一意にする。

| 設定 | Map Genでの通常プレビュー | レシピ実行時 |
| --- | --- | --- |
| 定数 | 指定した固定値 | 固定値。外部からの上書きはエラー |
| Min / Max、ランダムOFF | 仮の入力値 | 名前で渡された範囲内の数値。省略時は仮の入力値を既定値として使う |
| Min / Max、ランダムON | 仮の入力値 | 実行Seedと入力名から範囲内の値を決める。外部からの上書きはエラー |

「整数」をONにすると整数だけを扱う。ランダム整数はMinとMaxの両方を含む。
扱える入力は有限の-1,000,000,000〜1,000,000,000。Min≤Max、仮の値も範囲内にする。
入力を使う側のノードを選び、右の`PARAMETER INPUTS`で設定項目にNumber Inputを割り当てる。
例：Tile Samplerの`count_x`に整数入力、`scale`に小数入力を割り当てる。
接続は金色の線で表示する。割当中はその入力が直接設定の値を置き換える。「直接設定」に戻すと解除。
画像入力用のSource等へNumber Inputを配線することはできない。
接続先の許容範囲からMin/Maxがはみ出す場合や、整数項目に小数入力を割り当てる場合はエラーになる。

左の「実行時の値で試す」をONにするとランダム入力もSeedで評価する。「実行Seed」の`+1`で別の結果を試せる。
実行時はNoise・Tile Sampler・Connect Regionsの乱数もSeedとノードID・保存されたseedから派生する。
明示的に`seed`へ入力を割り当てたノードでは、割当値を優先する。同じ入力名は1回の実行中に同じ数値を使う。

3方式の入力を接続した編集サンプル：

```sh
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/examples/recipe_inputs.json
```

GUIなしでマップJSONを保存する例（保存先は任意のパス）：

```sh
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/examples/recipe_inputs.json --save /tmp/my-map.json
```

保存したJSONへSeedと`columns`の外部入力を渡して検証する例：

```sh
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph /tmp/my-map.json --seed 123 --input columns=7 --export tool/map_gen/output/input-test.png
```

`--input NAME=VALUE`は複数指定可能。範囲外・未知名・重複名を拒否する。`--input`だけの実行Seedは42。
`--seed`/`--input`は`--export`と併用する。指定しなければ通常の仮入力でプレビュー出力する。
`--save`は画像出力や実行入力と併用しない。旧`--export-recipe`も同じ完全なJSONを保存する互換名として使える。

保存形式は`schema_version=1`の完全なGraph JSON。実行用データを別途保存する必要はない。
共通ライブラリの`map_gen::load(path)`と`map_gen::generate(&graph, seed, &inputs)`をゲームも使用する。
生成中に編集用グラフを書き換えず、数値入力の解決や不要な枝の除外はメモリ内で行う。
同じJSON・入力・Seed・生成実装・実行環境での再現性を前提とする。

## 黒を透明にする

左の「画像加工」から`Black to Alpha`を追加し、`Source`へ画像を接続する。

```text
Shape / 加工したマスク → Black to Alpha → Output
Terrain Palette → Black to Alpha → Output
```

出力RGBが完全な黒（0,0,0）のピクセルだけをアルファ0にする。非黒ピクセルの色・既存の透明度・地形IDは維持する。
濃淡画像では8bit RGBへの変換後の黒を判定する。色付き画像ではTerrain PaletteのRGBを判定する。
プレビュー・サムネイルは市松模様の上に表示し、PNGはRGBAで保存する。
画像加工は透過化の前に行う。後段はOutput・Terrain Palette・Terrain Mergeに接続し、それ以外へ接続した場合は生成時にエラーになる。
MyRogueではアルファ0のマスの地形Entity配置を省略する（ベースgroundと外周boundaryは残る）。
サンプル：`examples/black_to_alpha.json`。透過処理自体もノードとして同じJSONに保存する。

## 地形カラー・IDをノードで決める

```text
ノイズ・床/壁などのマスク加工 → Terrain Palette → Output
```

左の「地形ID・出力」からTerrain Paletteを追加する。旧「地形カラー」チェックボックスは廃止。
各行で「上限」「ID」「色」を編集する。色ボタンからRGBを指定でき、「地形を追加」「削除」で分類数を変える。
上限は0〜1の昇順、最後は1、IDは一意。上限以下の最初の行に分類する（例：0.5は上限0.5の行）。
IDは英数字・`.`・`_`・`-`の文字列で最大128バイト。数字だけのIDも文字列として指定する。
カラーとIDはJSONのノードに保存され、サムネイル・プレビュー・PNGも同じ配色を使う。
カーソルのツールチップで生成値とIDを確認できる。
色は確認用で、ゲームでの意味を決めるのはID。同じ色でも違うIDを使え、色変更で地形の種類は変わらない。
Entityの画像や32色パレット自体は変更しない。

画像加工やマスク合成はTerrain Paletteの手前で行う。地形ID付き出力の後段はOutput・Black to Alpha・Terrain Mergeへ接続する。
地形ID付き画像をBlur/Blend等へ渡すとIDの意味が失われるため、生成時にエラーにする。
JSONの保存先は今までどおり1つでよく、色の凡例やIDの別ファイルは不要。

## 複数のTerrain Paletteを合成する

左の「地形ID・出力」から`Terrain Merge`を追加する。

```text
床マスク → Terrain Palette ─────────────────→ Background
壁マスク → Terrain Palette → Black to Alpha → Foreground
                                      Terrain Merge → Output
```

両方の入力に地形ID付き出力を接続する。前景が不透明な場所は前景のID・色、透明な場所は背景のID・色を使う。
前景の黒い背景を抜きたいときは、黒の表示色を設定し、Black to Alphaを挟む。
黒色だけでは透過扱いにならず、不透明な前景は背景全体を置き換える。
任意の`Mask`には濃淡画像を接続でき、値が`Mask cutoff`以上の場所だけ前景を採用する。未接続なら全域を対象にする。
透明なマスク・透明な前景は背景を残す。選ばれた入力のアルファを維持する。
これはIDを選ぶ合成で、色の補間や半透明色の混合は行わない。1マスは常に1つの地形IDになる。

異なるIDは出力のID表へ追加し、同じID・同じ色は共通化します。同じIDで色が異なる場合はエラーになるため、色を揃えるか別IDを使います。同じIDの境界接続方法と境界幅（`seam`）はForeground側で上書きします。この設定の上書きは前景ピクセルの透明度やMaskによる選択に関係なくID単位で行い、多段合成では最後のForegroundを採用します。
各Terrain Paletteは最大32種類、合成結果は最大256種類。合成結果をさらにTerrain Mergeへつなげば3つ以上も重ねられる。
両方のパレットと接続は、通常のJSON保存でそのまま残る。MyRogueも最終合成結果のIDを使うため、Luaで値域を再指定する必要はない。
一般のBlendへの地形ID付き接続は引き続きエラーになる。

8ノードの床＋壁サンプル：

```sh
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/examples/terrain_merge.json
cargo run -- --scenario data/scenarios/terrain_merge.lua --windowed
```

<a id="tile-boundaries"></a>

## 隣接マップの境界と3×3プレビュー

Terrain Paletteの各地形IDで境界方式と「境界幅（マス）」を選びます。設定はJSONの各bandの`seam`へ保存します。

| 境界方式 | 動作 |
| --- | --- |
| 切断 (`cut`) | 自分の地形を維持。壁や建物向け。既存JSONの既定値。 |
| モザイク (`mosaic`) | 境界へ近づくほど隣の地形を採用する確率を増やし、マス単位のまだらで接続。野原向け。 |
| 自然な塗りつなぎ (`fill`) | 連続した低周波ノイズで採用領域を決め、まとまった輪郭で接続。池・荒地向け。 |
| 近くの道へ接続 (`road`) | 境界幅の範囲で同じIDの道を探し、近い道筋同士を共有の開口へ4方向の経路で接続。 |

境界幅は1〜128マスで、生成時はタイル幅の半分までに制限します。地形ID・RGBの補間はせず、選んだ地形のID・色・アルファを維持します。透明マスは塗りつなぎで埋めません。切断・道は通常の塗りつなぎの対象外。異なる塗りつなぎ方式同士では`fill`を優先します。道の接続は地形の上に描くため、接続経路上の壁にも開口を作ります。相手の探索範囲に同じIDの道がない場合は延長しません。

```json
{ "max": 0.7, "id": "terrain.field", "color": [94, 122, 80],
  "seam": { "mode": "mosaic", "width": 6 } }
```

2D VIEWの「隣接3×3（9枚）」をONにすると、中央＋周囲8タイルを生成します。中心タイルX/Yと実行Seedを変えて、接続を確認できます。マス境界の線と各タイルの座標を重ねます。このモードは常に最終出力と実行Seedを使用し、PNG書出しも3×3画像になります。接続に使う外側の隣接タイルも生成してから切り出すため、周囲のタイルも単独生成と同じ結果になります。UI上の中心座標・表示切替はプレビュー設定で、JSONにはTerrainの境界設定を保存します。

```sh
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/examples/stitched_tiles.json
# 2D VIEWで「隣接3×3（9枚）」をON
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/examples/stitched_tiles.json --neighbors --seed 42 --export tool/map_gen/output/stitched_tiles.png
# 同じワールドの1タイルを生成（負の座標も可）
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/examples/stitched_tiles.json --tile-x 1 --tile-y 0 --seed 42 --export tool/map_gen/output/tile_1_0.png
```

共有APIの`world::generate_tile(graph, seed, inputs, [x, y])`と`world::generate_neighborhood(...)`が同じ接続処理を使います。同じSeed・座標・入力なら生成順によらず再現します。標準のプレビュー・本編は同じJSONを座標ごとのSeedで生成します。異なるJSONの出力同士をつなぐ共通APIは`world::stitch_tile`（座標→未接続Imageの表、中心の周囲8枚を供給）。解像度を揃え、共通IDの色・境界設定を一致させます。異なるIDは統合し、最大256種類。異なるJSONの割当を選ぶGUIは現時点ではありません。

本編のシナリオでは次のように指定します。JSON解像度とシナリオの幅・高さを同じにしてください。

```lua
map_generator = {
    json = "tool/map_gen/examples/stitched_tiles.json",
    tile = { 0, 0 }, -- ワールドのタイルX/Y。同じseedで隣は{1,0}など
    inputs = {},
},
```

`tile`指定時は外周にも生成地形を配置し、従来のシナリオ外周壁を自動配置しません。省略時は従来の単独生成と外周壁を維持します。実例は`data/scenarios/stitched_tiles.lua`です。これは指定タイルの生成機能で、歩行による隣接タイルへの切替・ワールド保存はまだ実装していません。

## 同じJSONをmy_rogueで使う

地形ID付きサンプルを編集する：

```sh
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/examples/game_map_ids.json
```

同じJSONを使うゲームを起動する：

```sh
cargo run -- --scenario data/scenarios/map_gen_ids.lua --windowed
```

GUIなしの確認は`--windowed`の代わりに`--check-data`を指定する。
Luaシナリオには次の設定だけで接続でき、値域や色を二重に指定する必要はない：

```lua
map_generator = {
    json = "tool/map_gen/examples/game_map_ids.json",
    inputs = {}, -- Number Input名で実行時の数値を渡す
}
```

JSONのIDをEntity定義ID（例：`terrain.road`、`obstacle.stone_wall`、`obstacle.wooden_door`）にすれば、そのIDでstatic Entityを生成する。
独自のIDを使う場合はLuaで対応を指定できる：`ids = { floor = "terrain.road", wall = "obstacle.stone_wall" }`。
省略したIDはEntity定義IDとして直接解釈する。存在しないID・static以外のID・JSONにない別名設定はエラー。
Entityの性質や行動は引き続きLua定義を使い、Rustに個別Entityの分岐を追加しない。

旧グレースケールJSONとLuaの`terrain = { { max=..., definition_id=... }, ... }`も引き続き使える。
旧サンプルは`game_map.json`と`data/scenarios/map_gen.lua`。Terrain Palette付きJSONではLuaの`terrain`を省略する。
`json`はゲームのルート（`--root`）からのパス。ルート外のファイルは拒否する。
生成Seedはシナリオの`seed`、外部入力は`inputs`を使う。
`map_generator`を省略した既存シナリオは従来どおり固定配置になる。

ゲームの寸法は現在5〜70×5〜38。正方形の生成画像全体をゲーム寸法へ最近傍で対応させる。
同じ解像度・寸法なら1ピクセル=1マス。縮小時は細い通路・扉が失われる場合がある。
外周はシナリオの`boundary`で閉じ、内側は`ground`の上へ指定地形を配置する。
`ground`と同じ地形は重複配置しない。追加の`spawns`を配置後、プレイヤー開始地点の通行可否を検査する。
開始地点の自動探索、全域の到達性保証、複数の名前付き出力、画像からの敵・アイテム自動配置は未対応。

## Partitionに入口・扉を追加する

```text
Partition ───────────────→ Entrance Candidates (Rooms)
    └→ Connect Regions ─→ Entrance Candidates (Corridors)
```

Connect Regionsの「元のブロックも出力」はOFFにし、Corridorsには通路だけを渡す。
Entrance Candidatesで「矩形モード」をONにして設定する：

| 設定 | 意味 |
| --- | --- |
| Marker width / px | 壁に沿う方向の幅 |
| Depth / px | 壁を貫く方向の奥行き |
| Outward offset / px | 部屋内側の縁から壁側へ移動する距離。0〜64、0.5などの小数も可 |
| 向かい合う入口を統合 | 対向する部屋から出た矩形が重なる場合、中心をまとめて1つの矩形にする |
| Min spacing / px | オフセット・統合後の候補中心同士の最小距離 |

上下左右の部屋境界から外向きを判定し、矩形を縦横に自動回転する。
正方形モードは従来のマーカーで、旧JSONも正方形・オフセット0として開く。
画像外へ出た矩形部分は切り落とす。複雑な角や斜めの壁では向きが曖昧になり、任意形状の扉配置を保証するものではない。
統合は向きが逆で移動後の矩形が重なる候補だけを対象にする。壁厚に合うオフセットと奥行きを指定する。
例えば2pxの仕切りを挟む部屋の縁から1.5px外側へ移すと、壁の中央へ揃えられる。

すぐ試せる10ノードのサンプル：

```sh
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/examples/partition_doors.json
cargo run -- --scenario data/scenarios/partition_doors.lua --windowed
```

サンプルは床と通路を最大値合成して0.5倍し、入口マスク（値1）を最大値合成する。
Terrain Paletteで0〜0.25を壁、0.25超〜0.75を床、0.75超〜1を木の扉へ分類する。
32×32・seed42では床676/壁340/扉8ピクセル。ゲームの外周はboundaryを優先する。
各ピクセルを1つのEntityへ変換するため、奥行き2の入口は2マス分の扉になる。1マスの扉ならDepthを1にする。
Entrance Candidates単体はマスク生成で、実際の開閉はMyRogueに生成した扉の既存Lua動作を使う。

## シェープを並べる

NODE LIBRARYから`Shape`と`Tile Sampler`を追加し、`Shape → Tile Sampler (Source) → Output`と接続する。
右のPROPERTIESでShapeの種類を選び、Tile Samplerの`Count X / Count Y`で横・縦の個数を設定する。
`Scale`はセルに対する画像サイズ、`Scale X / Scale Y`はその横・縦の追加倍率（各0.01〜1）。
`Scale random`は0で同じ大きさ、1で各コピーをほぼ0〜指定サイズへランダムに縮小する。
`Position random X / Y`は0で整列、1で各中心をセルの幅・高さの±半分までずらす。
`Rotation`に`Rotation random`の±角度を加え、画像中心を基準に回転する。
`Seed`で配置を再現し、隣の`+1`で別の配置を試せる。ランダム量を0にするとシードに関係なく整列する。

各セルに1個ずつ配置するため、コピー数はCount X×Count Y。入力はShape以外のノードも使える。
画像の外側は黒、重なりは最大値で合成。キャンバス端をまたぐコピーは反対側へ回り込む。
回転したShape自体が入力画像の枠を越える部分は切れるため、必要に応じShapeのScale X/Yを下げる。
地図のextentはShape/Tile Samplerに影響しない。

6×8個の四角をランダム配置するサンプル：

```sh
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/examples/shape_tiles.json
```

GUIなしで確認する場合は`--export tool/map_gen/output/shape_tiles.png`を追加する。

## 白いブロックを通路でつなぐ

```text
Shape → Tile Sampler → Connect Regions → Output
```

`Connect Regions`のSourceに白黒マスクを接続する。`Region cutoff`以上のピクセルを上下左右の4近傍でまとめ、各ブロック内の代表点を求める。
代表点間の距離から最小全域木を作り、N個のブロックをN−1本の線で全体がつながるように接続する。
代表点は重心に最も近いブロック内のピクセルなので、凹形状でも黒い穴を接続先にしない。

- `Width / px`：線の太さ1〜64。正方形ブラシで広げるため、斜めの線の見かけの太さは方向によって変わる。
- `Distortion`：歪みの強さ0〜0.25（画像の一辺に対する割合）。0なら直線。
- `Bend frequency`：線のうねりの頻度0.5〜8。
- `Seed`：うねりの形を決める。同じ設定・Seedで再現できる。
- `Region cutoff`：白いブロックとして検出するしきい値0.01〜1（既定0.5）。
- 「元のブロックも出力」：ONなら入力と白い通路を重ね、OFFなら通路だけを出力。

太さ1でも通路が上下左右に連続するよう描画する。通路は白（1）で、元画像を出す場合は入力値を保持して最大値で重ねる。
空画像・ブロック1個では新しい線は作らない。キャンバス端は反対側へ回り込まず、歪んだ線は画像内へ制限する。
ブロックは最大4096個まで。細かなノイズで上限を超えた場合はThreshold/Blurなどで入力を整理する。
通路は別のブロックを通過・交差することがあり、障害物回避やマップの出入口確保はこのノードの対象に含めない。

Widthなどの数値はPARAMETER INPUTSから入力ノードへ割り当てられ、プロジェクト保存・レシピ保存・実行Seedに対応する。

サンプル（12個の部屋と歪んだ通路、太さ用の整数入力）：

```sh
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/examples/connected_rooms.json
```

## マップ用の追加10ノード

左の`MAP OPERATORS`に追加。すべてマスク/濃淡画像として接続し、数値設定はPARAMETER INPUTSへ割り当て可能。
同じ設定のプロジェクト保存、生成レシピへの書出し・再読込に対応する。

- **Morphology**：Dilateで白い領域を広げ、Erodeで削る。二値化後、ユークリッド距離で半径を判定。半径0は二値化のみ。
- **Wall Outline**：元の白い床の外側だけへ壁を作る。画像の外へ出る壁は切れる。
- **Distance Map**：「白い領域の内側の距離」ONで白いセルから壁までの距離、OFFで各セルから白い領域までの距離。`Distance range`ピクセルで1になり、遠いほど白い。内部はユークリッド距離、画像外は黒として扱う。白がない場合、OFFは全白、ONは全黒。
- **Mask Scatter**：入力がcutoff以上の場所へ、中心間の最小距離を守って配置。Countは0〜4096、Radius 0は1ピクセル、1〜32は円形マーカー。円は入力マスク内へ切り抜く。間隔は中心同士に適用し、詰められない場合はCountより少なくなる。seedで配置を再現する。
- **Entrance Candidates**：Roomsの内側の縁とCorridorsの白が重なる部分をまとめ、接触領域ごとに代表点を1つ置く。Widthの正方形マーカーとSpacingで調整。Roomsには元の部屋、Corridorsには部屋の内側まで届く通路を渡す。扉の確定配置や壁の自動切り抜きはせず、候補マスクを返す。
- **Extra Connections**：Roomsの代表点から基本の最小全域木を求め、その辺を除いた短い組み合わせから追加する。Existing PathsとRoomsに完全に埋まる候補は除外。最大512領域、追加数0〜64（本数の上限）。候補試行は追加数×4（最低16、最大256）までなので、追加できる本数が足りない場合がある。入力に元のRoomsと既存の通路を渡す。道が他の部屋を通過することもあり、任意形状で必ず独立したループを作る保証はない。
- **Select Regions**：4近傍の領域をピクセル面積で選別。Min area〜Max areaを両端含む。最大のみの指定も面積条件を満たしたものから選び、残した領域では元の濃淡値を保持する。
- **Cellular Automata**：入力を二値化し、黒セルは周囲8近傍の白がBirth以上なら白に、白セルはSurvival以上なら白を維持。同一世代の画像からまとめて更新する。反復0〜64、誕生/生存閾値0〜8。画像外は黒。入力をNoise/Thresholdにするとseed付きの洞窟生成へ使える。
- **Partition**：各白い領域の外接矩形を長辺優先で分割し、指定幅の黒い仕切りを作る。最大8段、子区画の矩形寸法はMin child size以上。複雑な形状では実際の白い面積や接続性は保証しない。必要ならSelect RegionsやConnect Regionsを後段へつなぐ。
- **Reachable Area**：開始X/Yは出力画像のピクセル座標。開始点が白ならそこから4近傍で進める領域を残し、黒なら空画像。開始座標が解像度の外ならエラー。

共通の`Mask cutoff`は0.01〜1。距離・寸法・面積は出力画像のピクセル単位（マップのextentと独立）。
Morphology/Outline/Distance/Scatter/Entrances/Cellular/Partitionはマスクを作り、実際のEntity生成は行わない。
Scatter・Extra Connections・Partitionのseedも実行Seedから派生し、明示的なseed入力を優先する。

全10種類を試せるサンプル（追加通路の本数と壁幅の数値入力付き）：

```sh
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/examples/map_nodes_showcase.json
```

ノードをダブルクリックすると途中結果を確認できる。初期の最終出力は部屋＋基本通路＋追加通路。
サンプルのDistance Map→Mask Scatterは、壁から8ピクセル以上離れた場所へ配置する例。
全体表示で概要を見て、100%へ戻してから目的のノードへパンすると編集しやすい。
マップJSONはすべての枝を保存する。最終出力が使う枝だけを実行時に抽出する。

## 数値の扱い

画像は正方形の`f32`配列。解像度16〜1024（GUIの候補は64/128/256/512/1024）、最大64ノード。
RGB合成ではなく、0〜1の高さ・マスクの値として処理し、最後に表示色へ変換する。

- ノイズ値は`clamp((raw+1)/2,0,1)`。画像ごとの最小最大による自動正規化はしない。
- サンプリング座標は`pixel/(resolution-1)*extent`。解像度を変えても同じ範囲を生成する。
- Blendは`background + opacity*mask*(blend(background,foreground)-background)`。
- 乗算は`a*b`、スクリーンは`1-(1-a)*(1-b)`。
- オーバーレイは下地`a<=0.5`なら`2*a*b`、それ以外は`1-2*(1-a)*(1-b)`。
- Levelsは`clamp((v-0.5)*contrast+0.5+brightness,0,1)^(1/gamma)`。
- Blur/Warpは画像の端の値を延長して参照し、範囲外へ出ない。
- 地形の分類閾値・ID・RGB色はTerrain Paletteノードに保存する。入力の濃淡値を保ちながら地形IDの分類情報を付与する。

同じ設定・依存コミット・実行環境では再現できる。異なるCPU/コンパイラ間の浮動小数点ビット一致は保証しない。

## 構成とtrait

| ファイル | 責務 |
| --- | --- |
| `src/recipe.rs` | 名前付き数値入力・検証・Seed解決、実行用グラフの抽出、旧レシピ形式の読込 |
| `src/map_nodes.rs` | マスクの形態処理・距離・配置・入口・追加通路・選別・洞窟・分割・到達範囲 |
| `src/connections.rs` | 白い領域の検出・最小全域木・歪んだ通路と太さの生成 |
| `src/patterns.rs` | Shape・Tile Samplerの設定と生成処理 |
| `src/terrain.rs` | 地形の値域・ID・色と検証 |
| `src/graph.rs` | `ImageNode` trait、設定の保存形式、DAG検証、依存順の評価、共有ノードの評価結果キャッシュ、各画像処理 |
| `src/app.rs` | egui編集画面、配線・配置・履歴、プレビュー、生成ワーカー |
| `src/lib.rs` | GUI非依存の共通API、JSON保存・読込・生成 |
| `src/main.rs` | 起動・CLI、PNG出力 |

```rust
pub trait ImageNode {
    fn title(&self) -> &'static str;
    fn inputs(&self) -> &'static [&'static str];
    fn evaluate(
        &self,
        inputs: &[Arc<Image>],
        context: RenderContext,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Image>;
}
```

`NodeKind`はパラメータを持つ保存可能なenumで、このtraitを実装している。
`Node`はID・名前・配置・入力への参照、`Graph`は接続と実行順を管理する。
新しい種類を追加するときは`NodeKind`・画像処理・パラメータUI・カタログを拡張する。実行時に外部プラグインを読み込む仕組みはない。

生成は別スレッドで実行し、設定世代が古くなると行単位で中断する。古い結果は表示へ採用しない。
1回の評価内では共有入力を一度だけ計算するが、画像パラメータ・配線の変更時は全グラフを再評価する。
計算中は前回の画像を表示し、再計算中の表示を出す。配置・名前だけの変更では再評価しない。

## GUIなしの確認

```sh
cargo test --manifest-path tool/map_gen/Cargo.toml
cargo run --manifest-path tool/map_gen/Cargo.toml -- --export tool/map_gen/output/demo.png --terrain
cargo run --manifest-path tool/map_gen/Cargo.toml -- --save tool/map_gen/output/demo.json
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/output/demo.json --export tool/map_gen/output/gray.png
```

旧CLIの`--terrain`は既定のTerrain Paletteを追加する互換指定。新しいグラフはノードに色とIDを保存する。
`--graph`だけなら保存したグラフをGUIで開く。`--export`ならウィンドウなしで最終出力のPNGだけを保存する。JSON保存には`--save`を使う。
未知のフィールド、範囲外、存在しない参照、重複ID、循環接続は読込時に拒否する。
日本語フォントはmacOSのヒラギノ、LinuxのNoto Sans CJK、WindowsのMeiryoの既知パスを試す。任意の日本語フォントは`MAP_GEN_FONT`で指定できる。

## 現時点の範囲

カラー画像入力、PBRチャンネル、3Dプレビュー、サブグラフ、コピーペースト、GPUでのノード計算、変更ノードからの増分再評価は未実装。
保存形式はschema_version=1の独自JSONへ統一。旧recipe_version=1も読込可能。Substance Designerのファイル形式との互換性はない。
本編はLuaシナリオから同じJSONを読み、地形へ適用できる。Entity画像素材の生成は対象外。
