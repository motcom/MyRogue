> 元資料：my_rogue/document/braket_lib.md（2026-11-12 のスナップショット）。
> 本文の相対パス・現在の状態・確認結果は元プロジェクトについての記録です。作業先では版と状態を確認してください。

# bracket-lib の使い方

このプロジェクトでローグライクを作るための日本語ガイドです。
ファイル名は指定どおり `braket_lib.md`、ライブラリの正式名は **bracket-lib**、Rust のコード上の名前は **bracket_lib** です。

## 1. 対象バージョンと役割

2026-11-12 時点のプロジェクトの依存関係を対象にしています。

- 取得元：<https://github.com/amethyst/bracket-lib>
- ブランチ：`master`
- 使用コミット：`ce9124ad744553b2576e44df44e94b0f72faa8e8`
- パッケージのバージョン表記：`0.8.7`
- 描画：標準の OpenGL バックエンド

GitHub 版のバージョン表記も `0.8.7` なので、公開版との区別にはコミットを使います。
実際の取得版はプロジェクトの `Cargo.lock` に記録されています。

bracket-lib は、描画・入力・乱数・座標・視野・経路探索などを提供する Rust ライブラリです。
Neovim 固有のプラグインではありません。nvim で Rust を編集し、`cargo run` でゲームを起動します。
現在の設定では、ゲームは別のウィンドウに表示されます。

| 機能 | 主に使うもの |
| --- | --- |
| ウィンドウ作成 | `BTermBuilder` |
| ゲームループ | `GameState`, `main_loop` |
| 描画・入力情報 | `BTerm` |
| 座標・距離 | `Point`, `DistanceAlg` |
| 乱数・サイコロ | `RandomNumberGenerator` |
| 視野 | `field_of_view` |
| 経路探索 | `a_star_search`, `DijkstraMap` |

プレイヤーの HP、敵の行動、アイテム、ターンの進行、セーブなどはゲーム側で実装します。
ECS の採用は必須ではなく、まず構造体で状態を保持すれば十分です。

## 2. 導入・起動・更新

このプロジェクトの `Cargo.toml` には、すでに次の依存が入っています。

```toml
[dependencies]
bracket-lib = { git = "https://github.com/amethyst/bracket-lib", branch = "master" }
```

プロジェクトのルートで実行します。

```sh
cargo build             # ビルド
cargo run               # 起動
cargo fmt -- --check    # フォーマット確認
```

nvim 内なら `:terminal cargo run` でも起動できます。
初回は依存ライブラリの取得にネットワーク接続が必要です。

GitHub の更新を取り込む場合は、次を実行します。

```sh
cargo update -p bracket-lib
cargo build
```

通常のビルドでは `Cargo.lock` に固定された版を使います。
更新後は API や依存ライブラリが変わる可能性があるため、ビルドと起動を確認してください。

## 3. 現在の最小コードを読む

現在の `src/main.rs` は次の構成です。

```rust
use bracket_lib::prelude::*;

struct State;

impl GameState for State {
    fn tick(&mut self, ctx: &mut BTerm) {
        ctx.cls();
        ctx.print(2, 2, "Hello, roguelike!");
        ctx.set(40, 25, YELLOW, BLACK, to_cp437('@'));
    }
}

fn main() -> BError {
    let context = BTermBuilder::simple80x50().with_title("My Rogue").build()?;
    main_loop(context, State)
}
```

- `prelude::*`：よく使う型や関数をまとめて読み込みます。
- `State`：ゲームの状態です。今はデータを持っていません。
- `GameState`：毎フレーム呼ばれる `tick` を定義するトレイトです。
- `BTerm`：描画機能と入力情報を持つコンテキストです。
- `simple80x50()`：横 80 セル、縦 50 セルのコンソールを用意します。ピクセル数ではありません。
- `build()?`：構築に失敗した場合は、エラーを呼び出し元へ返します。
- `BError`：bracket-lib のエラーを扱う戻り値の型です。
- `main_loop`：状態を受け取り、イベント処理と `tick` の呼び出しを進めます。

`tick` の中で `loop` を回したり、入力待ちで停止したりしないようにします。
そのフレームの処理を行い、ライブラリへ制御を返します。

## 4. 描画の基本

座標の原点 `(0, 0)` は左上です。右へ行くと x、下へ行くと y が増えます。
80×50 の画面では、x は `0..80`、y は `0..50` の範囲です。

次は `tick` の中に書く断片です。

```rust
ctx.cls();
ctx.print(2, 2, "Hello!");
ctx.print_color(2, 3, WHITE, BLACK, "HP: 10 / 10");
ctx.set(10, 10, YELLOW, BLACK, to_cp437('@'));
```

| API | 意味 |
| --- | --- |
| `cls()` | 現在のコンソールを消去する |
| `print(x, y, text)` | 文字列を描く |
| `print_color(x, y, fg, bg, text)` | 前景色・背景色を指定して文字列を描く |
| `set(x, y, fg, bg, glyph)` | 1 セルを描く |
| `to_cp437('@')` | 文字を CP437 のグリフ番号へ変換する |

色には `WHITE`、`BLACK`、`YELLOW` などの定数や、`RGB::from_u8(255, 128, 0)` を使えます。
`cls()` で消去する構成では、地形・キャラクター・UI を毎フレーム描き直します。
同じセルに描くときは、後から描いた内容が手前になります。

標準フォントは CP437 用です。日本語の文字列を渡すだけで日本語表示できる構成ではありません。
このガイドの画面用文字列は英数字にしています。日本語 UI はフォントと文字の割り当てを含めて別途検討します。

## 5. 矢印キーでプレイヤーを動かす

以下は、`src/main.rs` 全体を置き換えて試せる例です。
現在のゲーム本体にはまだ反映していません。

```rust
use bracket_lib::prelude::*;

const WIDTH: i32 = 80;
const HEIGHT: i32 = 50;

struct State {
    player: Point,
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut BTerm) {
        match ctx.key {
            Some(VirtualKeyCode::Left) => self.player.x -= 1,
            Some(VirtualKeyCode::Right) => self.player.x += 1,
            Some(VirtualKeyCode::Up) => self.player.y -= 1,
            Some(VirtualKeyCode::Down) => self.player.y += 1,
            Some(VirtualKeyCode::Escape) => ctx.quitting = true,
            _ => {}
        }

        // 上の 2 行を説明表示用に空ける。
        self.player.x = self.player.x.clamp(0, WIDTH - 1);
        self.player.y = self.player.y.clamp(2, HEIGHT - 1);

        ctx.cls();
        ctx.print(1, 0, "Arrow keys: move / Esc: quit");
        ctx.set(self.player.x, self.player.y, YELLOW, BLACK, to_cp437('@'));
    }
}

fn main() -> BError {
    let context = BTermBuilder::simple80x50().with_title("My Rogue").build()?;
    let state = State {
        player: Point::new(WIDTH / 2, HEIGHT / 2),
    };
    main_loop(context, state)
}
```

`ctx.key` は `Option<VirtualKeyCode>` です。キー入力がある場合だけ `Some(...)` として処理します。
これは押しっぱなし状態を毎フレーム調べる仕組みとは異なります。連続移動の感触は OS のキーリピートにも影響されるので、必要に応じて入力状態と移動間隔を別途管理します。

ローグライクでは **描画の 1 フレームとゲームの 1 ターンを分ける**のが重要です。
敵の行動を無条件で `tick` に書くと、入力がなくても毎フレーム動きます。
入力を行動へ変換し、ゲーム側で「移動成功時にターンを消費する」「壁への移動は消費しない」などのルールを決めます。

リアルタイムの演出では `ctx.frame_time_ms` を使えます。単位はミリ秒で、秒にするには `1000.0` で割ります。

## 6. マップ・衝突判定・視野・経路探索

地形は例えば `Vec<Tile>` で保持し、座標と配列の添字を対応させます。

```text
添字 = y * 幅 + x
x = 添字 % 幅
y = 添字 / 幅
```

負の値を `usize` に変換する前に、必ず範囲内かを確認します。
移動では、移動先が範囲内か、壁でないか、他のキャラクターがいないかを確認してから座標を更新します。

視野と経路探索を使うには、自分のマップに次のトレイトを実装します。

| トレイト・メソッド | ライブラリへ伝える情報 |
| --- | --- |
| `Algorithm2D::dimensions` | マップの幅と高さ |
| `BaseMap::is_opaque` | 視線を遮るセルか。`true` なら遮る |
| `BaseMap::get_available_exits` | 移動できる隣接セルの添字と移動コスト |
| `BaseMap::get_pathing_distance` | ゴールまでの推定距離 |

以下は **単独で実行できる計算例**です。ウィンドウは作りません。
外周を壁にした部屋で視野と経路を計算し、標準出力へ結果を出します。

```rust
use bracket_lib::prelude::*;

struct Map {
    width: i32,
    height: i32,
    walls: Vec<bool>,
}

impl Algorithm2D for Map {
    fn dimensions(&self) -> Point {
        Point::new(self.width, self.height)
    }
}

impl BaseMap for Map {
    fn is_opaque(&self, idx: usize) -> bool {
        self.walls[idx]
    }

    fn get_available_exits(&self, idx: usize) -> SmallVec<[(usize, f32); 10]> {
        let p = self.index_to_point2d(idx);
        let mut exits = SmallVec::new();
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let next = Point::new(p.x + dx, p.y + dy);
            if self.in_bounds(next) {
                let next_idx = self.point2d_to_index(next);
                if !self.walls[next_idx] {
                    exits.push((next_idx, 1.0));
                }
            }
        }
        exits
    }

    fn get_pathing_distance(&self, idx1: usize, idx2: usize) -> f32 {
        let a = self.index_to_point2d(idx1);
        let b = self.index_to_point2d(idx2);
        ((a.x - b.x).abs() + (a.y - b.y).abs()) as f32
    }
}

fn main() {
    let mut map = Map {
        width: 20,
        height: 15,
        walls: vec![false; 20 * 15],
    };

    for y in 0..map.height {
        for x in 0..map.width {
            if x == 0 || y == 0 || x == map.width - 1 || y == map.height - 1 {
                let idx = map.point2d_to_index(Point::new(x, y));
                map.walls[idx] = true;
            }
        }
    }

    let player = Point::new(2, 2);
    let visible = field_of_view(player, 8, &map);
    println!("Visible cells: {}", visible.len());

    let start = map.point2d_to_index(player);
    let goal = map.point2d_to_index(Point::new(10, 8));
    let path = a_star_search(start, goal, &map);
    if path.success && path.steps.len() > 1 {
        let next = map.index_to_point2d(path.steps[1]);
        println!("Next step: {}, {}", next.x, next.y);
    }
}
```

`SmallVec` は隣接セルとコストを返す小さな配列で、`prelude` から使えます。
この例は上下左右だけの移動なので、推定距離にはマンハッタン距離を使っています。
`get_available_exits` に現在位置そのものを含めないようにします。

`field_of_view` の戻り値は見える座標の一覧です。移動や地形変更に合わせて更新します。
探索済みの場所を残すには「今見えるセル」と「過去に見たセル」を別々に保存します。
描画や添字変換に使う前には `map.in_bounds(point)` で範囲を確認すると安全です。

`path.steps` は始点を含む添字の列です。次に進むセルは、成功と長さを確認したうえで `steps[1]` を使います。
複数の敵が同じ目的地へ向かう場合は、距離場を計算する `DijkstraMap` も選択肢です。
壁以外の障害物や移動コストも、ゲームのルールに合わせてマップ側へ反映します。

## 7. 乱数と再現可能な生成

次は単独で実行できる例です。

```rust
use bracket_lib::prelude::*;

fn main() {
    let mut rng = RandomNumberGenerator::seeded(42);
    let x: i32 = rng.range(1, 79); // 1 以上、79 未満
    let damage = rng.roll_dice(2, 6); // 6 面ダイスを 2 回振る。2〜12
    println!("x={x}, damage={damage}");
}
```

`range(min, max)` は **下限を含み、上限を含みません**。
`RandomNumberGenerator::new()` なら固定シードを指定せず生成できます。
乱数生成器は通常 `State` などに保持し、抽選のたびに作り直さないようにします。
固定シードは地形生成のデバッグに便利です。同じ版・同じ処理順で乱数を使うと結果を再現しやすくなります。

## 8. 実装を進める順番

1. ウィンドウ表示と終了を確認する。
2. プレイヤーの座標を状態に持たせ、キー入力で動かす。
3. 床と壁を保存し、壁への移動を止める。
4. 入力・ゲームのターン更新・描画を分ける。
5. 視野と探索済み領域を追加する。
6. 敵・戦闘・アイテムを追加する。
7. 状態が増えたら `map.rs`、`player.rs`、`input.rs` などへ分割する。

## 9. よくあるつまずき

| 症状 | 確認すること |
| --- | --- |
| `bracket_lib` が見つからない | `Cargo.toml` の依存追加と、プロジェクトのルートでの実行を確認 |
| 初回ビルドで取得に失敗する | ネットワーク接続と Cargo キャッシュへの書き込み権限を確認 |
| 更新後に古い例がコンパイルできない | docs.rs の公開版と GitHub 版の差を確認 |
| プレイヤーが描画されない | 座標の範囲と描画順を確認。`cls()` は描画前に実行 |
| 入力がなくても敵が動く | 敵の更新を毎フレーム実行していないか確認 |
| A* で経路が見つからない | 始点・終点、壁、`get_available_exits` の実装を確認 |
| 日本語が表示されない | 標準の CP437 フォントでは日本語表示を想定していない |

## 10. 確認状況と参考資料

- 現在のゲーム本体：この Mac / Rust 1.99.1 でビルド済み。
- 本文のコード例：導入中のコミットを対象にコンパイルを確認。マップ計算例と乱数例は実行も確認。
- 未確認：ウィンドウの実表示、移動例の操作感、GUI の終了動作。
- 本文の移動・マップ・乱数の例は説明用で、ゲーム本体への追加実装はしていません。

参考資料は英語です。API の詳細は、導入中のコミットのソースと照合してください。

- [公式リポジトリ](https://github.com/amethyst/bracket-lib)
- [今回のコミットのソース](https://github.com/amethyst/bracket-lib/tree/ce9124ad744553b2576e44df44e94b0f72faa8e8)
- [公式利用ガイド](https://bfnightly.bracketproductions.com/bracket-lib/)
- [Rust Roguelike Tutorial](https://bfnightly.bracketproductions.com/rustbook/)
- [公開版の API ドキュメント（0.8.7）](https://docs.rs/bracket-lib/0.8.7/bracket_lib/)

利用ガイドとチュートリアルには古い記述や `rltk` という名前が出ることがあります。
このプロジェクトでは `bracket_lib::prelude::*` を使います。
