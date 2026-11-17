# Map Gen — Node Material Lab

Substance Designerのように画像処理ノードを接続して、ノイズ・マスク・高さマップを作る独立したRustアプリの初期版。
生成にbracket-noise、編集画面にeframe/eguiを使用する。本編の`my_rogue`には依存しない。
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
| 左のNODE LIBRARY | ボタンでノードを追加。検索で種類を絞り込む |
| 中央のGRAPH | 各ノードの正方形のサムネイルと接続を表示 |
| ノードをクリック | 選択し、右のPROPERTIESでパラメータ編集 |
| ノードの端子以外をドラッグ | 画像部分を含むどこでもノード移動。配置だけの変更ではノイズを再計算しない |
| ノードをダブルクリック | 下部の大きいプレビューへ表示を固定。別ノードの単クリックでは切り替わらない |
| 出力端子から入力端子へドラッグ | 接続。端子を順にクリックしてもよい |
| 入力端子を右クリック | その接続を解除。右側のINPUTSの×でも解除可能 |
| グラフ背景をドラッグ | パン |
| 背景上でホイール | カーソル位置を基準にズーム（スクロール量が正なら縮小、負なら拡大） |
| グラフ背景を右クリック | ノード追加メニュー |
| 全体表示 | 全ノードが入るように表示位置・倍率を調整 |
| Delete / Backspace | テキスト入力中でなければ選択ノードを削除 |
| Esc | 接続途中の線をキャンセル |
| Undo / Redo | パラメータ・接続・追加/削除・配置の変更を戻す/やり直す。連続したドラッグ操作はまとめる。最大100履歴 |
| Cmd+Z / Cmd+Shift+Z | テキスト入力中でなければUndo / Redo。Windows/LinuxはCtrlを使用 |
| 下部の2D VIEW | ダブルクリックしたノードの途中結果、または「最終出力」ボタンで出力を表示。初期高さは最大760（上のグラフ用に220を確保し、画面高さに合わせて調整）。画像全体をパネル内に収まる大きな正方形で表示する。境界をドラッグして高さを変更できる |
| 地形カラー | グレースケールと水・海岸・平地・岩山・雪の検証配色を切替 |
| プレビュー上のカーソル | セル座標と0〜1の値を表示 |
| このノードを最終出力にする | 任意のノードをPNG書出し対象にする |

右側の`SAVE / LOAD / EXPORT`からJSONを保存・読込し、PNGと同名のグラフJSONを書き出す。
既定の保存先はこのフォルダの`output/`。保存先は直接編集できる。保存・書出しボタンは同名ファイルを上書きする。
PNGは最終出力を現在の濃淡/地形カラー設定で出力する。途中結果の選択や画面の拡大はPNGへ含めない。
地形カラーの表示モード、パン・ズーム、選択ノード、Undo履歴はJSONへ保存しない。

## 実装済みノード

| ノード | 入力 | パラメータ・処理 |
| --- | --- | --- |
| Noise | なし | Simplex / Perlin / Value / Cellular / White、seed、周波数。前3種はFBMのoctaves・gain・lacunarity |
| Uniform | なし | 0〜1の一定値 |
| Blend | Background、Foreground、任意のMask | 通常・乗算・スクリーン・オーバーレイ・加算・減算・最小・最大、不透明度 |
| Levels | Source | コントラスト・明るさ・ガンマ |
| Invert | Source | `1-v`で反転 |
| Threshold | Source | 境界値、境界のぼかし幅。幅0なら二値化 |
| Blur | Source | ピクセル単位の半径0〜32。縦横に分離したボックスフィルター |
| Directional Warp | Source、Displacement | 変位画像の0.5を基準に斜め45度方向へ変位。強さ0〜0.25、双線形補間 |
| Output | Source | 入力画像を最終出力へ渡す |

Blendのマスク未接続は白（1）として扱う。黒で効果なし、白で不透明度どおりの効果。
未接続の必須入力があるノードはエラーにし、別の独立した正常ノードは引き続き評価する。
循環する配線は拒否し、元の接続を維持する。ノード削除時には関連する入力を切り離す。

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
- 地形カラーの閾値は現在0.38/0.43/0.70/0.86固定。カラー表示用であり、グラフの生成値は変えない。

同じ設定・依存コミット・実行環境では再現できる。異なるCPU/コンパイラ間の浮動小数点ビット一致は保証しない。

## 構成とtrait

| ファイル | 責務 |
| --- | --- |
| `src/graph.rs` | `ImageNode` trait、設定の保存形式、DAG検証、依存順の評価、共有ノードの評価結果キャッシュ、各画像処理 |
| `src/app.rs` | egui編集画面、配線・配置・履歴、プレビュー、生成ワーカー |
| `src/main.rs` | 起動・CLI、JSON/PNG入出力 |

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
cargo run --manifest-path tool/map_gen/Cargo.toml -- --graph tool/map_gen/output/demo.json --export tool/map_gen/output/gray.png
```

`--graph`だけなら保存したグラフをGUIで開く。`--export`ならウィンドウなしで最終出力のPNGとJSONを保存する。
未知のフィールド、範囲外、存在しない参照、重複ID、循環接続は読込時に拒否する。
日本語フォントはmacOSのヒラギノ、LinuxのNoto Sans CJK、WindowsのMeiryoの既知パスを試す。任意の日本語フォントは`MAP_GEN_FONT`で指定できる。

## 現時点の範囲

カラー画像入力、PBRチャンネル、3Dプレビュー、サブグラフ、複数ノード選択/コピーペースト、GPUでのノード計算、変更ノードからの増分再評価は未実装。
保存形式はschema_version=1の独自JSON。Substance Designerのファイル形式との互換性はない。
本編へのマップ適用・本編Entity素材の生成は行わない。
