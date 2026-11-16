# 制作のLua設定とRustの担当

## 設計方針

Entityの種類・設定・行動とゲームの調整ルールはLua、世界を安全に変更する共通処理はRustに置く。
既存の機能を組み合わせるEntityやレシピの追加は、Rustを変更せずLuaで完結させることを目標にする。
新しい仕組みそのものが必要な場合はRustの共通APIを追加し、個別Entityの名前やIDをRustの分岐へ増やさない。

| Luaの担当 | Rustの担当 |
| --- | --- |
| Entityの能力・道具機能レベル・材料・行動フック | 入力→更新→描画、座標とマップ、視野、Entityの生成と削除 |
| 制作レシピ、必要材料、要求道具・設備、推奨スキル | 到達可能な品の検索、収納の参照、条件の再検査 |
| 所要時間、完成品と個数、品質を付けるか | 材料の二重使用防止、消費と生成の一括確定 |
| 成功率と出来の補正値・境界、失敗損失率 | 検証済みの計算、再現可能な乱数、内部エラー時の巻き戻し |

現在、行動はEntityのLuaフックから既存のctx/APIを使って指示できる。すべてのゲーム機能がLua API化済みという意味ではない。
今回の制作はレシピと調整値をLuaへ接続した段階。NPCがLua行動から制作を開始する汎用API、独自の制作フック、スキル成長などは今後の拡張。

## 読み込み先

- レシピと制作の調整値：[data/production/recipes.lua](../../data/production/recipes.lua)
- 道具・材料：[data/entity_templates/item](../../data/entity_templates/item)
- 追加Entityの登録：[manifest.lua](../../data/entity_templates/manifest.lua)
- 読み込みと検査：LuaRuntime → CraftConfig → Game。

変更はゲームの再起動時に反映する。実行中の再読み込みは未実装。
Luaは既存と同じ実行時間・メモリ制限の下で評価し、OSやファイル書き込み用のAPIは公開しない。

## レシピの項目

ファイルは `rules` と `recipes` を持つテーブルを返す。付属の `recipe(...)` 関数は記述を短くするヘルパーで、Rustに固定された関数ではない。
配列へ次の形のテーブルを直接追加してもよい。

```lua
{
    id = "craft.example",
    name = "試作の棍棒を削る",
    ingredients = { { kind="item.wood", quantity=1 } },
    required_tools = { { function_name="cutting", level=1 } },
    required_facilities = { "work_surface" },
    required_skills = { { skill="wood_craft", recommended_level=0 } },
    resource_costs = {},
    outputs = { { kind="item.club", quantity=1 } },
    work_turns = 3,
    quality_enabled = true,
}
```

材料と道具要求は各1件以上。機能レベルは1〜20、材料・出力の個数と作業時間は1〜1000。
Entity ID、道具機能名、スキル名、設備名とレシピIDの重複は起動時に検査する。
推奨スキルは制作禁止条件ではない。複数指定なら最も不足している分野の差を使う。
現時点の燃料は `item.charcoal` など材料枠へ含める。`resource_costs` は空にし、資源メーターからの消費は行わない。

## 道具の機能レベル

```lua
item = {
    -- 長さ・重量など他の必須項目は既存の定義を参照。
    tool_functions = { "drilling", "cutting" },
    tool_levels = { drilling=2, cutting=1 },
}
```

`tool_levels` のキーには `tool_functions` にある機能だけを指定できる。
旧定義との互換性のため、機能はあるがレベル指定がなければLv1とする。
複数の道具のレベルは足し合わせない。複数機能の要求は、一つの複合道具でも別々の道具でも満たせる。

## 確率と出来の設定

付属の既定値：

```lua
rules = {
    success_base=60, skill_step=10, success_min=5, success_max=100,
    tool_step=5, tool_cap=10, material_step=5, material_cap=10,
    quality_thresholds={10,30,75,90,100},
    failure_material_percent=50,
}
```

- スキル補正：スキル差×`skill_step`。
- 道具補正：要求を上回るレベル×`tool_step`、最大`tool_cap`。複数要求なら最小の補正。
- 材料補正：材料の出来の段階×`material_step`、±`material_cap`以内。複数材料なら最小の補正。標準材料は0。
- 成功率：`success_base`に上記を加え、`success_min`〜`success_max`へ制限。
- 完成時の出来点：1d100に同じ補正を加える。`quality_thresholds`の5境界で−2〜＋3の6段階へ分ける。
- 失敗損失：材料枠ごとに必要量×`failure_material_percent`%を切上げて消費。別スタックごとの切上げはしない。

初期案の燃料レシピは木炭1個なので、既定の50%損失でも失敗時に1個を使う。
将来燃料を増やす場合に「燃料だけ成否にかかわらず全量」を保証する指定はまだない。
称号の文言と品目別の品質効果は現在Rustの共通処理。将来Luaへ公開する候補とする。

## 今回接続した範囲

12レシピ：棍棒、木柄、包帯、槍、メイス、ナイフ、盾、革の鎧、クロスボウボルト、焼肉、清潔な水、宝箱。
所持品・足元・隣接8マスと、その範囲の収納の中を利用する。収納は閉じていても利用可能。他人の所持品、範囲外、壁に埋まった品は使わない。
収納は現在の宝箱形式の平らな内容配列に対応し、箱の入れ子は扱わない。

開始時に使う品を自動選択し、作業ごとにその品の数量・距離・道具機能・設備を検査する。
選択が失われた場合は停止し、別の品へ自動で差し替えない。cで中止して開始し直すと再選択する。

武器・木柄・防具・箱は品質付き。武器はダメージ固定値へ品質段階を加減し、耐久品は最大耐久を1段階につき10%増減する。
標準品には称号も＋0も表示しない。品質の異なる品を自動でまとめない。
包帯・食品・飲料・弾薬は効果調整を決めるまで `quality_enabled=false` とし、標準品を出力する。
新造の宝箱は空。配置用の初期中身を複製しない。
完成品はインベントリ優先で受け取り、重量・容量・長さの制限で入らない分は足元へ置く。複数個の完成品は入る分だけ収納する。材料消費後の空き容量で、Luaの出力順に判定する。受け取りのための追加ターンは使わない。

## 検査と調整

```sh
cargo run -- --check-data
cargo test --test production --test lua_game
```

`--check-data` はLuaレシピ・Entityと画像の整合性も確認する。
新しいEntityにはテクスチャ登録が必要。追加14定義も専用32×32画像へ接続済み。原画と生成指示は `data/entity_tex/source/crafting/` に保存している。

レシピの値を変え、制作画面の成功率・必要材料・使用元を見て調整する。
現在の材料・道具レベル・設備などの条件を満たさないレシピはグレー表示する。選択すると不足理由を確認できる。推奨スキル未達でも挑戦できる場合は通常表示する。
実GUIと長期バランスの確認状況は [PROJECT_PROGRESS.md](../../PROJECT_PROGRESS.md) を参照。
