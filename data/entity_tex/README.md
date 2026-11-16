# Entity textures — Ash & Rust 32

現行 `data/entity_templates/manifest.lua` の全78定義に対応する32×32 PNG素材。既存64定義に加え、工具7・材料6・制作ストレージ1も専用画像を作成しゲームへ接続済み。追加14種はmanifestのsource_overrideとdedicated_texture=trueで原画へ対応する。
荒廃世界向けのピクセルアート試作。色は `document/color_thema/palette.json` の32色を使用する。
背景6種は視認性を保ちながら3色程度（石壁は2色）で草・土・水紋・石・板の質感を付けている。

## 確認

[一覧と透過の重ね合わせ見本](index.html) をブラウザで開く。
透過チェッカー・野原・道を切り替えられる。4倍表示は最近傍拡大。
[全78種の画像](preview.png) と[背景6種の見本](terrain_preview.png) も参照できる。
[野原・道との重ね合わせ画像](composite_preview.png) は実際のPNGをアルファ合成した見本。

- `movable/`：生存者とグール11種。背景透過。
- `item/`：武器・弾薬・食品・工具など51種。背景透過。
- `static/`：地形・壁・木・家具など15種。
- `atlas.png`：32×32を8列×10行に並べた256×320 RGBAアトラス。
- `manifest.json`：定義ID・日本語名・元Lua・PNG相対パス・アトラス座標・透過ピクセル数。
- `source/atlas32.png`：画像生成ツールで作成した64種の元原画。現在の背景6種は次の上書き原画を使う。
- `source/simple_terrain/`：背景6種の新原画。`recipe.json` に各生成プロンプト・色・対応IDを保存（built-in imagegenで生成）。
- `texture_rules.json`：道の接続グループ・下地と、各Entityの画像候補・固定シード。
- `source/prompt32.txt`：既存64種の原画の生成指示。
- `source/crafting/`：追加14種の個別原画。`recipe.json` は対応、`prompts.json` は内蔵imagegenへ渡した共通指示と各対象の指示。`source/crafting_aliases.json` は空になっており、仮流用は解除済み。
- [追加14種の見本](crafting_preview.png) / [地形との合成見本](crafting_composite_preview.png)：上段は工具7、下段は材料6と制作ストレージ。最近傍8倍表示。

野原・道・池・荒地・石壁・木壁の6種は全面不透明。
残り72種は背景透過の切り抜き。アルファは0/255の二値で、補間による縁のにじみを避ける。
表示時は地形→固定物・設備→アイテム→生物の順にアルファ合成する。
PNGをRGBAで読み込み、最近傍補間で拡大し、透明部分に背景色を塗らない。
透明部分には下のタイルがそのまま見える。

## 再処理

ImageMagickの `magick` が必要。Pythonは画像処理コマンドの実行とメタデータ作成を行う。

```sh
python3 tools/import_entity_textures.py data/entity_tex/source/atlas32.png
```

背景6種だけ再処理する場合は、それ以外の72種のPNGを保つ次のコマンドを使う。

```sh
python3 tools/import_entity_textures.py --only-simple
```

追加14種のみ再処理する場合（既存64種を保持）：

```sh
python3 tools/import_entity_textures.py --only-crafting
```

PNGを変更せず、登録・一覧・合成見本を再検査/更新する場合：

```sh
python3 tools/import_entity_textures.py --validate-only
```

全体再処理でも `source/simple_terrain/recipe.json` に登録した6種の原画を優先し、古い複雑な背景へ戻さない。各背景は3色以内・模様の面積がrecipeの対象別上限以下であることを検査する。色と上限は `palette_ids`・`max_detail_pixels` に保存する。

既存の原画は8×8に切り出す。追加14種の個別原画は透明余白で正方形に揃えて縦横比を保ち、32×32へ最近傍縮小、アルファ二値化、32色への減色を行う。16×16を経由しない。
全面タイルの透過部分は素材に合うパレット色で埋める。
全画像の寸法・パレット・二値アルファ・空画像の有無・透過の有無を検査する。
合成見本では、透明ピクセルの位置で下の地形と出力のRGBAが一致することも検査する。
再実行はこのディレクトリの生成物を上書きする。

## 現在の範囲

既定のゲーム描画はPNG。起動時に各PNGを読み込み、32×32・パレット内RGB・二値アルファ・非空を検査する。
日本語UIは別コンソールへ重ね、視野外・所持品はワールドへ表示しない。
`cargo run -- --ascii` で従来の文字描画へ切り替えられる。
素材は32×32、表示は既存80×50レイアウト全体の画面倍率に従う（画面上の物理ピクセル数はウィンドウとDPIに依存）。
既存64種についてゲーム画面で初期表示・透過重ね合わせ・日本語UIとの併用を確認済み。追加14種はPNGと地形合成見本の目視・自動検査まで確認し、実GUIでの表示は未確認。移動・開閉などの操作確認は未実施。
扉・宝箱は閉じた状態の基本画像。開いた扉は幅を狭めて地面を露出し、開いた宝箱には印を重ねる。開いた状態・部位欠損・死体・方向別・アニメーションは今後の追加対象。

## 道の接続と画像候補

道は上下左右の同じ接続グループへ幅16pxの帯を伸ばし、端・横道・縦道・曲がり角・T字・十字を切り替える。斜め隣接も調べ、広い道や広場の内側は地面を埋める。道のない側は `terrain.field` の草地を下地に使う。接続は見た目だけで、通行・移動コストはLuaの設定を変えない。池など別地形が道に重なった場合は、最上位の地形で隣接判定する。

`texture_rules.json` の `sets` はEntity定義ID別の設定。`connect_group` が一致する地形同士がつながり、`background` は下地の基本画像ID。専用の方向別PNGを揃える必要はなく、各候補から接続形状を起動時に作る。

同じ役割の柄を増やす場合は、32×32・指定32色・二値透過の候補PNGを保存し、対象の `variants` 配列にパスを追加する。次は今後ファイルを作成してから登録する例（`road-02.png` は現時点では未作成）。

```json
"terrain.road": {
  "connect_group": "road",
  "background": "terrain.field",
  "variants": ["static/terrain.road.png", "variants/road-02.png"]
}
```

候補は `seed`・役割・座標で選ばれる。同じ設定と場所では、再描画・ズーム・起動し直しても同じ柄になる。ゲームの戦闘乱数は消費しない。地形の接続用候補と下地は全面不透明にする。未存在ファイル・不正サイズ・パレット外の色は起動時に拒否する。現在の実データは各1候補で、柄違いのPNG追加は今後の作業。
