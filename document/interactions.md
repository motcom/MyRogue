# インタラクトとアイテム詳細

通常画面で `e` を押すと方向選択になる。hjklyubn・矢印で隣接マス、`.` / Spaceで足元を指定し、行動メニューを開く。同じマスに複数の対象がある場合は、対象名と行動名を列で表示する。j/k・上下矢印で選び、Enterで実行、Escで戻る。視野外・隣接していないマスは操作できない。

`g` は足元の品を拾う。足元に複数の品があれば名前と個数の一覧を開く。j/k・上下矢印で選択し、Enterで選んだ一行の全個数を拾う。足元の品が一つのときはすぐ拾う。一覧を開く、選ぶ、Escで閉じる操作はターンを消費しない。

`s` で固定物への攻撃方向を選ぶ。hjklyubn・矢印で隣接マスを指定し、破壊可能な固定物が一つなら攻撃する。複数なら一覧からj/k・矢印とEnterで対象を選ぶ。Escで中止。攻撃を実行すると一ターン進む。

`o` は「開く」の専用操作。足元と隣接8マスのうち、現在の状態でLuaの `id = "open"` が使える地面上のEntityだけを数える。一つなら即座に実行し、複数ならhjklyubn/矢印で方向、`.` / Spaceで足元を選ぶ。指定マスに一つなら実行、同じマスに複数なら名前・ID付きの一覧からj/k・Enterで選択する。Escで中止。所持品、視野外、遠方、破壊済み、既に開いてopenが無効の物は含めない。対象なしや選択/中止では時間を進めず、開く時だけ既存のLua処理を実行する。扉・宝箱の種類やタグで決め打ちせず、窓など新しいEntityでもopenを登録すれば対応する。

方向選択と行動メニューのポップアップは、マップの上に別のコンソールで重ねる。背景は不透明度60%の黒で背後のマップを透かし、文字と枠線は不透明に保つ。閉じた後はポップアップ層を透明に消去する。

所持品では `w` が数字キー登録・防具着脱、`e` が詳細説明。詳細には個数・耐久・寸法・性能・説明を表示する。長い説明はj/k・矢印・PageUp/Downでスクロールし、e/Escで元の所持品へ戻る。行動があるアイテムは詳細からEnterで行動メニューを開ける。

メニュー表示・選択・説明の閲覧では時間を進めない。拾得は一ターン、Luaに登録した行動は変更が成功した場合に `consumes_turn` の設定に従って進める。既定は一ターン。実行前に対象・距離・視野・現在の状態を再確認する。

## Luaの登録

Entity定義のトップレベルに `interactions`、`hooks` に `on_interact` を登録する。movable / static / itemで共通の形式。

```lua
initial_state = { opened = false },
interactions = {
    { id = "open", name = "開く", when_state = { opened = false } },
    { id = "close", name = "閉じる", when_state = { opened = true } },
},
hooks = {
    on_interact = function(self, ctx, event)
        local opened = event.interaction == "open"
        return {
            state = { opened = opened },
            effects = {
                { type = "set_collision", target = self.id,
                  movement = not opened, sight = not opened },
                { type = "set_glyph", target = self.id,
                  glyph = opened and "/" or "+" },
                { type = "message", message = opened and "開きました" or "閉じました" },
            },
        }
    end,
},
```

各行動の `id` は定義内で重複不可、`name` は表示名（制御文字なし、1〜48文字）。登録は最大32件。`when_state` は省略でき、指定した全てのキーが `self.state` の値と一致する場合だけ表示する。条件の値は文字列・数値・真偽値。ネストした条件は使わない。`consumes_turn = false` を指定すると実行後も時間を進めない。

`event.user` はプレイヤーID、`event.interaction` は選択した行動ID。`event.position` は対象の位置で、所持品の場合はプレイヤーの位置。`state` は既存の仕組みと同様に全体を置き換えるため、残したい状態は戻り値にも含める。インタラクトでは `action` を返さず、`effects` と `state` を返す。

追加した効果：

| type | 引数 | 挙動 |
| --- | --- | --- |
| `set_collision` | `target`, `movement`, `sight` | 自身の通行・視線の遮断を変更。movement=trueにするマスに他の可動体・通行を妨げる物がある場合は失敗する |
| `set_glyph` | `target`, `glyph` | 自身の表示を印字可能なASCII一文字に変更。色・layerは定義を維持する |
| `message` | `message` | ゲーム内ログへ表示する |
| `open_container` | `target` | 自身の中身選択画面を開く。containerタグのあるItemが対象 |

通行・視線・表示は個体ごとに変更し、同じ定義の別の扉には影響しない。複数効果の途中で失敗した場合は、その呼び出し全体の状態・表示・衝突・生成アイテム・ログを巻き戻し、ターンを消費しない。開いた扉の位置に誰かがいる場合は閉じられない。

## 試験用の配置

初期位置 `(8,10)` の西側、`x=2〜6 / y=5〜9` に石壁と道の床からなる建物を配置した。東側の入口 `(6,8)` に木の扉、室内 `(4,7)` に宝箱がある。屋外の `(9,9)` にも宝箱を配置し、初期位置から `e → u` で操作できる。

- [木の扉](../data/entity_templates/static/wooden_door.lua)：Static。閉じた状態は `+` で通行・視線を遮り、開くと `/` で両方通す。
- [宝箱](../data/entity_templates/item/treasure_chest.lua)：拾って持ち運べるItem。地面の宝箱には登録行動に加え「拾う」を表示する。足元なら通常のgでも拾える。拾得時は通常の所持重量・容量・長さの制限を確認する。

マップに配置される宝箱には、生成時に異なる2〜4品目が入る。品目ごとの抽選でまずTier 1〜4を65・26・8・1の重みで選び、次にそのティアの候補から品目と数量を選ぶ。例えば包帯は1〜3個、拳銃弾は6〜12発、武器は1個。取得後も残りを箱に保持する。制作した宝箱は空で生成する。

開くとティアと個数を表示する。j/k・上下矢印で移動し、Spaceでチェックを切り替え、Enterでチェックした行の個数すべてをまとめて取得する。複数行を選べる。取得は直接所持品へ移し、一回の確定につき一ターン。選択や中止は無料。未選択分は箱に残り、Escで取得せず閉じる。空の箱は「宝箱は空です」と表示する。

選択した品の重量・容量・長さをまとめて再検査し、一つでも上限を超えればすべて箱に残す。失敗時は選択を維持するので、Spaceで選び直せる。取得後も一覧を開いたまま残りを選べる。開いている箱の「中身を見る」は時間を進めず一覧を開く。地面の箱はEscで通常画面へ、所持中の箱は詳細へ戻る。

Luaの `item.contents` に初期内容を登録する。個体生成時にコピーし、その後はItemEntityのcontentsに残数を保持する。取得した品を再生成したり、開け直して補充したりしない。拾得・設置・Gameの複製でも残数を保持する。

```lua
item = {
    -- 通常の寸法・素材などに加えて指定する
    stack_limit = 1,
    contents = {
        { definition_id = "item.bread", quantity = 2 },
        { definition_id = "item.bandage", quantity = 2 },
        { definition_id = "ammo.pistol", quantity = 10 },
    },
},
-- on_interactから返す効果。state.opened=trueで開いた状態を保持する。
-- effects = { { type = "open_container", target = self.id } }
```

contentsは最大100行、各quantityは1〜1000の整数。参照先はItemのみで、初期中身のある箱を別の箱へ入れる設定は拒否する。中身のある箱はstack_limit=1必須。取得時は品物のstack_limitごとにスタックを分割する。スナップショットのcontentsでも各行の残数を参照でき、取り出し済みの行はquantity=0。

所持重量には箱の中身も含める。所持容量には箱の外寸の容量を使い、中身を外へ取り出した分は通常の所持容量に追加する。自由な収納・預け入れ、行内の個数指定、箱の入れ子は今回追加していない。
