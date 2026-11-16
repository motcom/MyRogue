# Entity の Lua 定義と行動関数

この文書は `data/entity_templates/` の共通フォーマットと、本ゲームの Lua API を示す。
初回 Rust 実装で mlua / Lua 5.4 から読み込む。実装範囲は [設計との対応](implementation.md) を参照。
外部 Lua ライブラリの API ではなく、本ゲームが提供する ctx とイベントの形式。

## LuaとRustの分担

Entityの種類・能力・行動・制作レシピと調整値はLua、世界の整合性を守る共通処理はRustへ置く。既存機能の組合せならLuaのみで追加・調整できることを目標とする。新機能が必要な場合はRustの共通APIを広げる。[制作の具体的な分担と現状](production/lua.md#設計方針)を参照。すべてのEntity機能のLua API化が完了したという意味ではない。

## ファイル構成

一つの `.lua` ファイルが一つの定義テーブルを `return` する。
`data/entity_templates/manifest.lua` が読み込み対象のファイル一覧。追加時はこの一覧にも登録する。
各ファイルを複製して `id` を変更できる。manifest 自体は Entity 定義ではない。

| テンプレート | category | 用途 |
| --- | --- | --- |
| `data/entity_templates/player.lua` | `movable` | 生存者プレイヤーの具体例 |
| `data/entity_templates/static.lua` | `static` | 野原の具体例 |
| `data/entity_templates/item.lua` | `item` | ナイフの具体例 |

Rust の `Entity` enum は `Movable` / `Static` / `Item` の三種類で `match` する。
設定上の種類が増えても enum の variant は増やさず、定義 ID で区別する。
プレイヤーも movable とし、操作対象の個体 ID を `Game.player_id` が参照する。

## entity_document に基づく具体定義

プレイヤーの操作は `document/entity_document/player_entity.md` を優先する。参照元は `document/entity_document/movable_entity.md`、`static_entity.md`、`item_entity.md`。
現在の定義は78件。下表は既存28件で、以前の追加36件は後述。制作の追加14件（工具7・材料6・収納1）は [Item仕様](entity_document/item_entity.md) を参照。威力・射程・耐久・重量などの値はゲーム用の仮設定。

| 種類 | 定義 |
| --- | --- |
| movable（4） | 生存者、グール ワーカー、グール ウォーリア、グール アーチャー |
| static（6） | 野原、道、石壁、木、池、木の扉 |
| item（資料の10件） | ナイフ、棍棒、グロック、クロスボウ、SKS、包帯、消毒薬、注射器、パン、ミネラルウォーター |
| 防具 item（1） | 革の鎧 |
| 補助 item（5） | 拳銃弾、クロスボウボルト、ライフル弾、木材、宝箱 |
| 既存案から保持（2） | 万力付き作業台、ハンマー |

- `movable/`：ワーカーは近接攻撃、ウォーリアは高い耐久と棍棒、アーチャーはクロスボウとボルトを持つ。攻撃可能なら攻撃し、そうでなければ接近。敵も経路もなければ待機する。
- `static/`：道・石壁・木・池。木は破壊後に木材を二個生成する。池は泳ぎ未対応のため通行不可。
- `item/`：近接・遠距離武器、消耗品、弾薬、木材。武器の使用は Rust への攻撃要求を返す。
- 包帯は選択した出血部位を100ターンで止血し、消毒薬はwound_contaminationを取り除く。注射器は内容未指定のため、今回は架空の回復薬入りとして設定。回復条件を満たさない場合には消費しない。
- パンは三栄養、ミネラルウォーターは水分を回復する。使用者自身への飲食に限定する。

survivor と ghoul は敵対、ghoul 同士は味方という初期ルールを想定する。
敵対判定・攻撃・回復・移動・消費は Rust 側へ接続済み。
既存案の万力付き作業台とハンマーも別ファイルで保持する。炉や金床の具体定義は今後追加する。

## 共通フォーマット

| キー | 内容 |
| --- | --- |
| `schema_version` | この形式では整数 `2`（ダメージダイス形式） |
| `id` | 定義の文字列 ID。例 `monster.ghoul_worker`。全定義間で重複不可 |
| `category` | `movable` / `static` / `item` のいずれか |
| `common.name` | 表示名 |
| `common.description` | カーソルモードに表示する説明文（省略可）。未指定なら未登録の案内を表示 |
| `common.appearance` | `glyph`、RGB の `fg` / `bg`、描画順 `layer` |
| `common.collision` | `movement` は通行を妨げるか、`sight` は視線を遮るか |
| `common.durability` | `false`（共通耐久なし）または `{ max = 正の整数 }` |
| `common.tags` | 検索・分類用の文字列配列 |
| `initial_state` | 個体ごとの Lua 状態の初期値 |
| 種類別テーブル | category と同名のテーブルを一つだけ持つ |
| `hooks` | イベント名から Lua 関数への対応。不要なイベントは省略可 |
| `interactions` | 任意。状態条件付きの行動メニュー登録。[インタラクト仕様](interactions.md)を参照 |

`id` は種類の ID、`self.id` は生成した個体の ID。別のものとして扱う。
位置や所持者は定義に固定せず、生成する際に Rust 側から指定する。
`initial_state` は個体ごとに深いコピーを作り、共有しない。
`name` と `description` は同梱の日本語ビットマップフォントで表示する。新しい文字を追加した場合は `src/assets/generate_font.swift` でフォント画像と対応表を再生成する。
プレイヤーの `controls[";"] = { type = "open_inspect" }` でカーソルモードを開く。
`controls.o = { type = "open" }` は足元・隣接8マスの `open` インタラクト専用操作。一対象なら即実行、複数なら方向で選ぶ。[インタラクト仕様](interactions.md)を参照。
`glyph` は初期案では表示可能な ASCII 一文字に限定する。
`layer` の目安は地面 0、障害物・設備 10、アイテム 20、生物 30。

### 種類別の項目

- `movable`：`role`、`controller`、`faction`、全体の初期最大HP `hp` と六部位名の配列 `body`、能力値 `attributes`、`skills`、所持容量 `inventory`、`senses`、`attack`、初期所持品 `loadout`、水分 `needs`、入力制御用 `controls`。
- `static`：`role`（terrain / obstacle / facility）、提供する `facilities`、初期 `resources`、経路コスト `movement_cost`（省略時1、有限数0.01〜1000）。
- `item`：`length_mm`、`volume_ml`、`mass_g`、`stack_limit`、`materials`、`tool_functions`、初期 `resources`、任意の `weapon`・`use`、食料・飲料の `nutrition`。

長さは mm、容量は ml、重量は g。item の数値は一個あたり。
movable の `hp` を生命力として扱い、共通 durability は `false` にする。
最大 HP から現在 HP を初期化する。初期の死亡条件・攻撃処理は [戦闘ルール](../rule/combat.md) に記載する。
`controller = "lua"` は Lua が行動を選び、`"input"` は入力された Action を実行する。
`loadout` は `{ definition_id, quantity, equipped?, grip? }` の配列。個体生成時に一度だけ所持品を生成する。
`equipped = true` は持ち方を自動指定する。`grip` でright/left/bothを指定でき、手が競合する初期装備は拒否する。装備する品のquantityは1。非装備のquantityは合計個数で、必要ならスタック上限ごとに分割する。
`needs` はhydrationのみの `{ max, initial }`。10ターンごとに1減少する。satietyは廃止し、指定を拒否する。
`controls` はキーから入力命令への対応。h/j/k/l と y/u/b/n で八方向移動する。
p/i/m は製作・所持品・マップの画面を開く命令で、画面を開くだけではターンを消費しない。
`weapon` は mode / range / base_pv / strength_cap / accuracy_bonus と damage・弾薬設定を持つ。
近接・遠距離とも damage = `{ dice, sides, bonus }` を必須とする。補正は damage.bonus に含める。弾薬は `common.tags` に `"ammunition"` を持つ通常のItemで、武器の ammo_definition_id から参照する。弾薬独自のダメージ値は持たない。
以前の遠距離設定から移行する場合は、弾薬の item.ammunition.damage に weapon.damage_bonus を合算したダイスを weapon.damage に移し、item.ammunition と weapon.damage_bonus を削除する。数値一つの旧形式は受け付けない。
任意の `item.armor = { slot = "body", av = 1, dv = 0 }` は防具設定。slotはhead/body/arms/hands/legs/feet（省略body）、avは0〜100、dvは-100〜100の整数。武器・盾以外の防具は手と別枠へ装着し、各部位一品。着用中の防具と使用可能な手の盾のAV・DVを合算する。耐久0の品は加算しない。部位平均・盾ブロックは未実装。
movable の attack も同じダイス形式と base_pv / strength_cap を持ち、近接射程は1。
movable と static の combat に base_av、movable には dv_bonus / accuracy_bonus も設定する。
[戦闘ルール](../rule/combat.md) の命中・貫通・ダメージを Rust で処理する。
`magazine_capacity` がある遠距離武器は未装填で生成する。r の `reload` で所持弾薬を弾倉へ移し、射撃時に弾倉から `ammo_per_attack` 発を消費する。装填数は個体の `loaded_ammo` に保持する。省略した既存定義は所持弾薬の直接消費を継続する。
`use` は消耗品の効果パラメータで、実際の効果の選択は hooks.on_use に記述する。
食料・飲料の `nutrition` は `{ protein, carbohydrate, vitamin }`。各値は一個あたりの栄養ポイントで、
三つ全てを `u32` の範囲の整数で指定する。食品一個の量と個体の栄養値を区別し、個体は初期50・上限100。水分は別に保持する。
具体的な型・摂取処理・値の範囲は [食料ルール](../rule/food.md) に記載する。
`role` は分類であり、具体的なモンスター名・木の種類などは `id` で増やす。

static テンプレートから次のように設定を変えて地形も定義する。

| 定義の例 | role | 通行を妨げる | 視線を遮る | layer | facilities |
| --- | --- | --- | --- | --- | --- |
| `terrain.field`（野原） | terrain | false | false | 0 | 空配列 |
| `terrain.wasteland`（荒地） | terrain | false | false | 0 | 空配列 |
| `obstacle.tree`（木） | obstacle | true | true | 10 | 空配列 |
| `obstacle.rock`（岩） | obstacle | true | true | 10 | 空配列 |
| `obstacle.wall`（壁） | obstacle | true | true | 10 | 空配列 |
| `facility.forge`（鍛冶炉） | facility | true | false | 10 | forge_heat |

これらは本ゲーム向けの初期設定例。地面は durability を false にし、
壊せる障害物・設備は正の最大耐久を設定する。炉の加熱には資源条件の確認も必要。

道具機能は `class.pu` の `ItemProp` と同じ 13 種類。
設備条件は `work_surface` / `clamping` / `anvil` / `forge_heat`。
資源名は `fuel` / `electricity` / `welding_material`、消費単位・量は未決定。
金属加工の材料・技能・設備の条件はレシピ側で要求する。

## Lua 関数の共通形式

```lua
on_turn = function(self, ctx, event)
    local count = (self.state.turns or 0) + 1
    return {
        state = { turns = count },
        effects = {},
        action = { type = "wait" }, -- movable の Lua 制御 on_turn のみ
    }
end
```

`self` は呼び出し時点の個体情報のコピー。`self.config` は定義データ
（hooks を除く）、`self.state` は前回保存した状態、`self.position` は配置位置。
所持中の item は `self.position == nil`、所持者は `self.carrier_id` で参照する。
参照用の個体情報は category を持ち、movable は `hp = { current, max }` と `body[部位名] = "healthy" | "bleeding" | "heavy_bleeding" | "severed"`、
`needs[種類名] = { current, max }`、`statuses[状態ID] = 値` を公開する案。
摂取した栄養の蓄積値は movable の `nutrition = { protein, carbohydrate, vitamin }` として公開する案。
状態値が false / nil / 0 のときは非アクティブとして扱う。
Lua のコピーを直接変更してもゲーム本体へは反映しない。変更は戻り値で指定する。
関数の外に置く可変変数や upvalue に個体の状態を保存せず、`state` に保存する。

戻り値はテーブル（何もしない場合 `{}`）。`nil` は `{}` と同じ扱い。
`state` は省略時に維持、指定時に丸ごと置換。`{}` を返せば空にできる。
保存できるのは真偽値・有限の数値・文字列・それらのテーブル。
テーブルのキーは文字列または正の整数とし、関数・userdata・循環参照は保存しない。
関数そのものは保存せず、ロードした定義の hooks と個体の state を対応させる。

| hook | 呼び出すタイミング | event の内容 |
| --- | --- | --- |
| `on_spawn` | 個体を配置した直後に一度 | 空テーブル |
| `on_turn` | 時間を消費するゲームターンに一度 | `turn` |
| `on_use` | 使用要求を受けたとき | `user`、任意の `target`。包帯では選択した `body_part` |
| `on_damage` | Rust が被害を確定した後。破壊時も呼ぶ | `source`、`amount` |
| `on_destroy` | 耐久切れや死亡による削除の直前に一度 | `source`、`reason` |
| `on_ability` | 超能力などの発動要求を受けたとき | `user`、`ability_id`、任意の `target` |
| `on_status` | 状態異常のターン処理時 | `status_id`、`value` |
| `on_interact` | 行動メニューから操作を実行したとき | `user`、`interaction`、`position` |

`on_damage` は適用済みの被害への反応を選ぶ。被害量の補正用ではない。
既存図の HP・Psykick・Status の function 部分も、これらのイベントから Lua へ振り分ける。
状態異常等の種類の定義形式・発動条件・消費コストは別途具体化する。
全 Entity が同じイベント形式を使い、固定物や所持品にも必要なイベントを届ける。

### 参照用 ctx API

コロン呼び出しを使う。戻り値はコピーで、ゲーム状態を書き換える API は公開しない。

| 呼び出し | 戻り値・意味 |
| --- | --- |
| `ctx:entity(id)` | 個体情報、存在しなければ `nil` |
| `ctx:entities_at(position)` | その位置の個体情報の配列。地面も含む |
| `ctx:nearest_hostile(id, range)` | 視認できる範囲内の最も近い敵。いなければ `nil` |
| `ctx:adjacent(a, b)` | 同じ z で八方向に隣接しているか |
| `ctx:can_attack(actor, target, weapon?)` | 攻撃できるか。射程・視線・所持・耐久・弾薬も確認。武器省略時は装備武器、使えなければ素手の近接攻撃を試す |
| `ctx:movement_cost(position)` | 固定Entityの優先順から決めたマスの経路コスト。マップ外は `nil`。通行可否とは別 |
| `ctx:step_towards(actor, target)` | 重み付きDijkstraで求めた通行可能な経路上の次の位置。到達不能・同じ位置・対象不在・次の位置が占有されている場合は `nil` |
| `ctx:random_int(min, max)` | 将来用。初回実装では未公開。戦闘の乱数は Rust 側で処理 |

八方向移動に合わせ、隣接判定は同じ z のチェビシェフ距離を使う。経路は `static.movement_cost` による合計コストが最小になるよう選ぶ。縦横・斜めとも移動先のコストを一回加算する。固定中のbracket-libのDijkstraには不均一コストへの制約が記載されているため、ゲーム側の優先度付きキューで重み付きDijkstraを実行する。対象から逆向きに探索し、候補の「一歩のコスト＋残りの最小コスト」で選択する。同点は既定の八方向の列挙順で決める。
同じマスで固定Entityが重なる場合は描画layer、次にEntity IDの大きい順で採用する。固定Entityがなければコスト1。詳細は[固定Entityの仕様](entity_document/static_entity.md#移動コスト)を参照する。地形コストは移動時間を変えない。脚の切断による4/8ターンの遅延は別に適用する。
プレイヤー・敵とも、隣接する縦横の片側または両側に壁があっても斜め移動・斜め近接攻撃を許可する。移動先そのものの壁・占有・マップ外判定は維持する。
同距離の候補は個体 ID 順で決定する。射撃は距離だけでなく視線も確認する。
敵対関係・視界・通行判定は Rust のルールを通す。Lua はその結果から戦術を選ぶ。

### action と effects

インタラクト用に `set_collision`、`set_glyph`、`message` を追加した。引数と制限は[インタラクト仕様](interactions.md)を参照する。

- `action`：movable の Lua 制御 `on_turn` から一つだけ選ぶ。`wait`、`move`（`position`）、`attack`（`target`）、`reload` に対応する。`ctx:can_reload(actor_id)` で装填の可否を照会できる。
- `effects`：全種類の hook から返せる効果の配列。初期案は `spawn`（`definition_id`, `position`, `quantity`）、`damage`（`target`, `amount`）、`heal`（同じ）、`consume_item`（`target`, `quantity`）。今回の具体定義用に以下も追加する案。
  - `weapon_attack`：`attacker`, `target`, `weapon`。使用者が所持し使用可能な手に装備する武器での攻撃要求。威力・射程・消費弾薬は武器設定から算出する。
  - `apply_bandage`：`target`, `body_part`。包帯のon_useから返し、使用者本人が所持する包帯で出血中・未処置の一部位へ処置を開始する。期間はsourceのitem.use.amountを使う。consume_itemと一つの効果列として返す。
  - `remove_status`：`target`, `status_id`。アクティブな状態を取り除く。
  - `restore_need`：`target`, `need`, `amount`。hydrationだけを最大値まで回復する。
  - `gain_nutrition`：`target`, `nutrition`。三栄養を個体へ加算し、それぞれ100に丸める。食品は栄養不足の項目があるときgain_nutritionとconsume_itemを一括で返す。

attack の威力は Rust が設定から決定し、Lua は攻撃対象を選ぶ。初期ルールでは技能補正は行わない。
weapon_attack でも弾薬消費と被害を一括適用する。対象不在・射程外・弾薬不足なら両方適用しない。
回復アイテムは使用者自身または隣接する生きた movable を対象とし、使用者の所持と残量を検証する。
heal は全体の最大HPまで、restore_need は最大値までに丸める。部位の負傷段階は回復しない。
`damage` / `heal` の `body_part` は廃止し、指定を拒否する。
`spawn.quantity` は item のみ複数可。movable / static は 1 とする。
未登録の定義、存在しない対象、不正な量、範囲外などは Rust が拒否する。
使っただけで item が消費されることはなく、消耗品は `consume_item` を返す。

例：木の `on_destroy` による木材生成（`item.wood` はカタログに登録済み）。

```lua
on_destroy = function(self, ctx, event)
    return {
        effects = {
            { type = "spawn", definition_id = "item.wood", position = self.position, quantity = 2 },
        },
    }
end
```

### Rust 側の処理順

1. `input` で操作を Action にする。時間を消費する場合にだけターンを進める。
2. `update` 内で Entity enum を `match` し、種類に応じた Lua hook を呼ぶ。
3. 個体 ID 順に処理する。一つの hook が返した state・effects・action はまとめて検証し、全て有効なら effects の配列順、action の順に適用して state を保存する。
4. 適用直前にも通行・攻撃距離・対象の生存・所持数を確認する。自身を消す effects と移動の併用など、互いに矛盾する結果も拒否する。
5. そこで発生したイベントはキューに積んで順に処理し、Lua の再帰呼び出しを避ける。destroy 対象は一度だけ通知し、その後削除する。通常ターンの処理対象は開始時に固定し、生成された個体の on_turn は次ターンからにする。
6. `render` で更新後の状態を描画する。Lua の行動関数は描画中には呼ばない。

入力制御の movable でも `on_turn` の state・effects は処理するが、action は禁止する。
Lua 制御で行動が省略された場合は待機とする。static / item も action は禁止。
未定義 hook は何もしない。Lua エラーや不正な戻り値はその hook の変更を破棄し、
定義 ID・個体 ID・hook 名を記録する。Lua 制御の行動失敗は待機とする。
実行量・イベント数の上限を設け、Lua エラーはメッセージに記録する。上限値は [実装の対応資料](implementation.md) を参照。

## 確認と残作業

`luajit tools/check_entity_templates.lua` で manifest 内の定義形式・装備と弾薬の参照を確認する。
`luajit tools/test_entity_behaviors.lua` で AI・キー設定・地形・武器・回復品・飲食の Lua 単体テストを実行する。
`luajit tools/test_food_rules.lua` で栄養値の u32 境界・不正値・飲食時の栄養要求を確認する。
個別ファイル群を引数に指定して確認することもできる（参照先も同時に指定する）。この検査は主要項目を対象とし、
Lua 検査は主要項目を対象とする。Rust 側でも型・数値範囲・参照・hook 戻り値を検証するが、全ての未知キーの拒否には未対応。
Lua 単体検査に加え、`cargo test` で Rust と Lua の接続を確認する。
`cargo run -- --check-data` で読み込み、`cargo run -- --simulate 20` で GUI なしのターン進行を確認できる。
資源メーター消費を要する制作・保存復元・ctx:random_int・探索済み地形の記憶表示などは未実装。制作スキルによる成否・出来の判定は接続済み。日本語描画と現在の視野による遮蔽は実装済み。GUI の確認範囲は進捗資料を参照。

### 武器のクリティカル設定

装備可能な全武器（銃・クロスボウを含む）の `item.weapon.critical_value` は必須の整数2〜12。例：`critical_value = 10`。生物への貫通後の正の被害に対し2D6を振り、値以上なら未切断の六部位から一つを選び負傷を一段階進める。判定・部位抽選の乱数はRustで管理する。詳細は[戦闘ルール](../rule/combat.md#全体hp部位の負傷とクリティカル)を参照。

movable定義は `hp = 60, body = { "head", "body", "left_arm", "right_arm", "left_leg", "right_leg" }` の形式に変更した。旧形式の部位HPのマップは受け付けない。

### 左右の手・利き手と持ち方

movableの `dominant_hand = "right"` または `"left"` で利き手を指定する（省略right）。初期所持品の `grip = "right" | "left" | "both"` で手を指定できる。quantityは1にする。既存の `equipped = true` は自動指定として利用でき、片手武器なら利き手、両手/両対応なら両手に装備する。同じ手を重複占有する初期設定、不正な持ち方は読み込み時に拒否する。

全武器は `item.weapon.hands = "one" | "two" | "either"` を必須とする。例：

```lua
-- 棍棒（既存のmode/damage/base_pv等と併記）
hands = "either",
two_hand_pv_bonus = 1,
-- 拳銃
hands = "either",
one_hand_range_penalty = 2,
one_hand_accuracy_penalty = 1,
-- ライフル
hands = "two",
```

`two_hand_pv_bonus` は両対応の近接武器のみ0〜30、省略0。片手では加算しない。片手補正は両対応武器のみ設定でき、射程低下は0以上・基礎射程未満、命中低下は0〜100の整数、省略0。逆手のみでの使用は合計ダメージ3/4（端数切捨て）で、PV自体は逆手補正で下げない。

参照スナップショットに `equipment = { right_hand, left_hand, active, dominant_hand }`、`movement_turns`（1/4/8）、`bleeding_damage`（部位ごとの毎ターン被害の合算）を追加した。空の手や使用可能な武器がないactiveはnull。両手持ちはright_handとleft_handが同じEntity IDになる。Luaの `ctx:can_attack` / `ctx:can_reload` にも欠損した手・実効射程・持ち方を反映する。武器攻撃には手への装備が必要。

部位の出血は1、大出血は2、切断は4を毎ターン末に合算し、全体HPから減らす。片脚切断は移動に4ターン、両脚切断は8ターン。切断した手に装備したアイテムは使用できない。回復薬・汎用状態除去は部位の負傷段階を治療しない。

### 包帯・止血と装備対象の制限

手に装備できるのはweaponを持つ武器と `common.tags` に `"shield"` を含む盾のみ。盾は片手用。包帯・食料・工具は手に装備しない。初期loadoutにも同じ制限を適用する。装備選択UIはアイテムのhandsと手の欠損に合わせて候補を絞り、装備中の場合だけ解除を追加する。

包帯の定義は `item.use = { kind = "bandage", amount = 100 }`。amountは正の整数で、処置開始からの世界ターン数。on_useへ選択したbody_partを渡し、`apply_bandage`と一個の`consume_item`を返す。sourceは対象本人が所持する包帯、targetは生存する本人、部位は出血中かつ未処置であることをホストで再検査する。失敗した効果列は包帯数・処置状態・乱数を巻き戻す。

参照用個体情報に `bleeding[部位名] = 毎ターン被害`、`bandages[部位名] = 処置完了予定の絶対ターン`、`stopped_bleeding = { 部位名, ... }` を追加した。bodyの負傷段階は止血完了後も保持する。包帯中のクリティカルで段階は進むが、完了予定は変えない。止血後のクリティカルでは保持した段階から進め、止血済みを解除する。切断状態は止血しても戻さない。

処置中は継続出血し、100ターン目の出血計算後に止血する。巻いた包帯の表示は自動で消え、部位は止血済みとなる。包帯の装備や汎用bleedingステータスの除去には使わない。

### 数字キー登録と防具

所持品w→1〜9・0で武器・盾または消耗品を登録する。武器・盾は選んだgripで装備し一ターン、消耗品の登録は無料。通常画面の数字キーで持ち替え・使用する。防具はwで直接着脱し、数字登録しない。初期loadoutではequipped=trueで防具を装着する。gripの指定・装着部位の重複は拒否する。

スナップショットの `armor` は装着部位からItem IDへのマップ。`quick_slots` は十要素の配列（1〜9、0の順）、空きはnull。武器・盾は `{kind="equipment", item=ID, grip="right"|"left"|"both"}`、消耗品は `{kind="consumable", definition_id="bandage"}`。武器は置く・削除で登録解除。消耗品は同種の別スタック・補充品にも登録が残る。

### 宝箱の中身

`item.contents = { { definition_id = "item.bread", quantity = 2 } }`で初期内容を設定する。生成時に個体へコピーし、スナップショットのcontentsに残数を保持する。containerタグのItemのon_interactで `open_container` 効果を返し、state.opened=trueとして中身選択画面を開く。複数選択・一括取得・容量検査・残数更新はRustで処理する。詳細と制限は[インタラクト仕様](interactions.md)を参照。

### entity_document の追加定義とAPI（2026-11-15）

manifestに36件を追加し合計64件とした。近接武器6（大鉈を含む）・ライフル2・食品飲料8・ビタミン剤1・投擲2・固定物8・盾1・グール8。

- `item.weapon` の近接rangeは1〜100。2以上はfの敵選択へ共通接続し、射線と装備した手を検査する。隣接敵への方向入力も攻撃になる。
- `item.throwable = { range=6, radius=2, power=24, kind="explosive" }` で投擲品を定義する。kindはexplosive/flash、rangeは1〜20、radiusは2、powerは1〜100。weaponとの併用は拒否する。
- 効果量は `power * (radius + 1 - distance) / (radius + 1)` の整数切捨て。距離はチェビシェフ距離で壁による射線遮蔽も検査する。flashのpowerは行動不能の世界ターン数。
- `item.use.kind="nutrition"` はnutrition必須。食品・ビタミン剤はgain_nutritionとconsume_itemを返す。水分は変えない。増やせる栄養が全て100なら何も返さない。
- `action={type="ghoul_turn", behavior="runner"}` をLua制御のon_turnから返せる。behaviorはrunner/butcher/guard/howler/scavenger/stalker/brute/rifleman。実際の移動・戦闘・拾得はRustが検査する。
- 可動個体の参照スナップショットへ `stunned_until`、`alert_position`、`alert_until` を追加。閃光中はon_turnを実行しない。最終目撃位置の通知だけでは現在の敵位置は分からない。

制作レシピと成否・出来の調整値は `data/production/recipes.lua` に定義する。材料・道具機能レベル・推奨スキル・設備・時間・出力をLuaから登録できる。詳細は [制作のLua設定](production/lua.md)。燃料は木炭など実体のある材料として指定し、資源メーターからの消費は未対応。

使用アイテム（回復品・栄養剤・食料・飲料・投擲品）は所持品Enterまたは種類単位の数字登録から使用する。throwableも数字登録に対応。槍はfまたは所持品Enterから照準を開く。

### テスト用シナリオ

既定で `data/scenarios/entity_test.lua` を読み込む。従来の `data/scenario.lua` は `--normal-map` で選択する。`--test-map` は互換用として同じテストマップを選ぶ。
シナリオの任意項目 `labels = { {position={x=3,y=2,z=0},text="近接武器"} }` は区画案内を表示する。
最大100件、各1〜70文字、制御文字なし、テキスト全体がマップ内に収まる位置に限定し、視野内の文字だけ描画する。
案内文字は衝突・視線・経路コストを持たない。Entityの配置や設定とは独立している。
配置と操作は[Entityテストマップ](entity_test_map.md)を参照。

### レベル・経験値・技能成長（2026-11-16）

敵の `movable.experience_reward`、武器の `item.weapon.skill`、共通effect `practice_skill` を追加。調整値は `data/progression.lua`。名前・成長条件・効果・未実装の専用操作とスナップショット項目は[成長仕様](progression.md)を参照。
