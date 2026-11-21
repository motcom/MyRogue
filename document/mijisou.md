# パラメータの使用状況と未実装項目

2026-11-16、消毒薬を削除した後の77定義・現行12制作レシピと、コンパイル対象のRustコードを確認した結果。
「値が存在する」「成長する」「画面に出る」と「ゲームの判定に効果がある」を分ける。独自Lua・外部の `--root` 設定で追加される使い方は今回の確認範囲に含めない。

- **使用中**：現在の操作・判定・表示に接続済み。
- **一部使用**：保持・成長・表示、または特定条件だけに接続。未実装の効果を併記する。
- **現行未使用／未接続**：標準データに出番がない、または判定処理がない。将来用の共通APIが存在する場合は別記する。

## 四つの能力と正気度

| 表示名・Luaキー | 状態 | 現在の効果・未接続部分 |
| --- | --- | --- |
| 筋力 `kinryoku` | 使用中 | 素手・近接武器のPVへ能力補正。武器の `strength_cap` で上限を制限。筋力そのものをダメージダイスへ加算せず、所持上限も別設定 |
| 敏捷 `subayasa` | 使用中 | 攻撃の命中補正、DV、同じ世界ターン内の攻撃モーションの表示順。実際の先攻判定・行動回数・歩行速度を変える仕組みはない |
| 器用さ `kiyousa` | 使用中 | 能力補正を近接・素手・射撃の命中へ加算。制作成功率へ能力補正×5ポイント。出来の抽選は変えない |
| 精神力 `seishinryoku` | 一部使用 | 共通サイキックAPIの威力と正気度被害の軽減へ能力補正。標準データの具体的な能力・発動操作・精神攻撃する敵は未実装 |
| 正気度 `sanity` / `max_sanity` | 一部使用 | 現在値/上限の表示、レベルごと上限1〜10増加、共通Lua effectによる被害、0でグール化・ゲームオーバー。標準データに被害の発生源・回復操作はまだない |

確認先：[生成・戦闘](../src/game/world.rs)、[武器PV](../src/game/equipment.rs)、[DV](../src/game/statistics.rs)、[成長](../src/game/progression.rs)、[モーション](../src/ui/animations.rs)、[表示](../src/ui/properties.rs)。
体力・知力・社交は削除済み。能力補正は `floor((値-8)/2)`。成長・正気度判定の詳細は[成長仕様](progression.md#能力スタミナ正気度)。最大HP・最大正気度は各1〜10、最大スタミナは1〜5をレベルごと独立に抽選し、現在値も同量増やす。

## 技能

| 表示名・Luaキー | 状態 | 使用・成長と効果 |
| --- | --- | --- |
| 近接武器 `melee_weapon` | 使用中 | 生物への有効な近接・素手攻撃で練習。5ごとに命中+1 |
| ライフル `rifle` | 使用中 | 現行では拳銃を含む銃の技能。発射で練習。5ごとに命中+1 |
| 弓 `bow` | 使用中 | 現行クロスボウの発射で練習。5ごとに命中+1 |
| 盾 `shield` | 使用中 | 使用可能な盾を持って攻撃を受けると練習。盾が使える間、5ごとにDV+1 |
| 投擲 `touteki` | 一部使用 | 成立した投擲で練習し、成長・表示する。威力・射程・命中への数値補正はない |
| 木工 `wood_craft` | 使用中 | 現行レシピあり。作業で練習し、技能差が成功率・出来に反映 |
| 金属加工 `metal_craft` | 使用中 | 現行レシピあり。同上 |
| 裁縫 `saihou` | 使用中 | 現行レシピあり。同上 |
| 料理 `cooking` | 使用中 | 現行レシピあり。同上。品質なしレシピでも成功率に使う |
| 電子工作 `denshi_craft` | 現行未使用 | 保持・表示と共通の制作/練習APIはあるが、現在の12レシピに電子工作の指定はない |
| 解錠 `picking` | 未接続 | 保持・表示・共通の練習APIはある。専用の操作・成功判定はない |
| ハッキング `hacking` | 未接続 | 同上 |

確認先：[技能・練習](../src/game/progression.rs)、[戦闘での練習](../src/game/world.rs)、[投擲](../src/game/item_actions.rs)、[制作判定](../src/game/crafting.rs)、[現行レシピ](../data/production/recipes.lua)。
外した攻撃も有効な対象への試行なら練習する。無効操作・画面閲覧では練習しない。投擲の被害は `throwable.power` で決まり、投擲技能を掛けていない。

## 生命・生活・成長の状態

| 項目 | 状態・用途 |
| --- | --- |
| 全体HP・最大HP、部位の負傷段階 | 使用中。被害・死亡・出血・部位欠損・止血・手足の使用制限。部位別HPは持たない |
| 包帯の処置期限・止血済み部位 | 使用中。巻いた部位の出血ダメージを即座に止め、100世界ターン後に処置完了 |
| 水分 `hydration` | 使用中。消費・回復・表示 |
| 三栄養 `protein` / `carbohydrate` / `vitamin` | 使用中。自然回復、消費、走行回復、風邪・肺炎の判定 |
| スタミナ・走行モード・走行の移動端数 | 使用中。走行2マス/世界ターン、1マスでスタミナ5消費、回復と栄養消費。初期上限100、レベルごと1〜5増え、成長後の上限まで回復 |
| 病気の経過・回復・肺炎時の移動カウンタ | 使用中。病状の進行・回復、移動時の被害 |
| レベル・経験値・練習回数 | 使用中。敵撃破の経験値、成長と次までの表示。死亡後も保持 |
| `stunned_until` / `next_move_turn` | 使用中。閃光の行動不能、脚の欠損による移動遅延 |
| `alert_position` / `alert_until` | 使用中。敵の通知・追跡先と期限 |
| 超能力 `psykick` | 標準データでは未接続。`Game::use_ability` はhookと残り使用回数を処理するが、現在のプレイヤーに能力定義も操作キーもない |
| `status` | 共通の保持・更新・hookは接続済み。通常の病気はcold / pneumoniaを利用。任意の状態名に自動で効果が付くわけではない |

確認先：[栄養](../src/game/nutrition.rs)、[包帯](../src/game/treatment.rs)、[敵行動](../src/game/enemies.rs)、[超能力API](../src/game/mod.rs)、[プレイヤー定義](../data/entity_templates/player.lua)。

## Entityの設定

| 設定 | 使用状況 |
| --- | --- |
| `id` / `category` / `schema_version` / 各種定義参照 | 使用中。識別・種類の検査・生成・読込 |
| `common.name` / `description` | 使用中。名前・ログ・詳細説明 |
| `appearance.layer` | 使用中。地形・固定物・品物・生物の重なり順 |
| `appearance.glyph` / `fg` / `bg` | 条件付き使用。ASCII表示・Luaによる記号変更・PNGがない時の代替。通常の専用PNGの色はPNG自体が決める |
| `collision.movement` / `collision.sight` | 使用中。通行・視野・射線・爆発の遮蔽 |
| `durability` | 使用中。壊れる物の耐久・破壊判定。耐久なしの物には数値の減少なし |
| `tags` | 種類により使用。`shield`・`container`・`ammunition` はRustの判定あり。任意のタグに自動のゲーム効果はない |
| `movable.role` / `controller` / `faction` | 使用中。種類・操作主体・敵味方。現在の標準敵対ルールはsurvivorとghoulの組み合わせ |
| `movable.hp` / `body` / `dominant_hand` | 使用中。初期HP、部位、利き手・逆手補正 |
| `movable.senses.sight_range` | 使用中。視野・敵の索敵・照準 |
| `movable.attack` | 使用中。素手などの既定攻撃のダイス・PV・射程・筋力上限。装備武器の攻撃には武器側の値を使う |
| `movable.combat` | 使用中。基本AV・DV補正・命中補正 |
| `movable.inventory` | 使用中。持てる長さ・総容量・総重量の制限。能力値からの自動算出ではない |
| `movable.loadout` / `controls` / `needs.hydration` | 使用中。初期所持と装備、キー入力、初期水分と上限 |
| `movable.experience_reward` | 使用中。敵の撃破報酬 |
| `static.movement_cost` | 使用中。経路の選択。移動一回のターン数を変える値ではない |
| `static.facilities` | 使用中。制作で設備要求を照合。標準の現行レシピ/設備ではwork_surfaceを利用。炉・金床の具体的な実装はない |
| `static.combat.base_av` | 使用中。固定物への攻撃で装甲として参照 |
| `static.combat.dv_bonus` / `accuracy_bonus` | 現行未使用。固定物は回避せず、攻撃主体にもならない |
| `item.length_mm` / `volume_ml` / `mass_g` / `stack_limit` | 使用中。生成数量・所持・収納・重量/容量とスタック制限 |
| `item.materials` → 個体 `zairyou` | 保持のみ。木・金属などの素材属性による自動の耐久/威力/制作補正はない。制作材料はレシピの定義IDで選ぶ |
| `item.tool_functions` / `tool_levels` | 使用中。制作で機能・要求レベルを判定。個体の `nouryoku` にも複製されるが、判定は定義を直接参照する |
| `item.weapon` / `armor` / `throwable` | 使用中。下記の条件差に注意 |
| `item.use` / `nutrition` | 使用中。使用の種類・水分・HP・栄養の回復など。消毒薬と傷の汚染は削除済み |
| `item.use.amount` | 使用中。HP/水分の回復量、包帯では処置に要するターン数（標準100）。種類によって意味が異なる |
| `item.use.status_id` | 消毒薬削除後の標準定義では未使用。状態削除の汎用hook/APIは維持 |
| `item.contents` | 使用中。箱の中身の定義・数量、取得、制作の材料/工具の参照 |
| `static.resources` / `item.resources` | 保持のみ。現行データは空。燃料・電力・溶接材を資源メーターから減らす実装はない |
| `initial_state` / `hooks` / `interactions` | 使用中。個体のLua状態・イベント・行動メニュー、扉や箱の開閉条件とターン消費 |

武器の `mode`、ダイス、PV、射程、命中補正、クリティカル閾値、持ち方、弾薬・弾倉、技能の指定は使用中。
`strength_cap` と `two_hand_pv_bonus` は近接のPV用で、射撃武器のPVには加算しない。`one_hand_range_penalty` / `one_hand_accuracy_penalty` は該当する持ち方だけに適用する。
投擲の `range` / `radius` / `power` / `kind` は全て使用中。爆発は36/24/12、閃光は3/2/1ターンが標準値。
確認先：[定義](../src/model/config.rs)、[生成・攻撃・経路](../src/game/world.rs)、[装備](../src/game/equipment.rs)、[収納](../src/game/containers.rs)、[画像](../src/media/textures.rs)、[Lua](../src/scripting.rs)。

## 制作と成長の調整値

| 設定 | 使用状況 |
| --- | --- |
| 制作の `ingredients` / `outputs` / `work_turns` | 使用中。材料・生成品・数量・作業時間 |
| `required_tools` / `required_facilities` / `required_skills` | 使用中。工具の機能/レベル・設備、推奨技能との差で判定 |
| `quality_enabled` | 使用中。出来の抽選を有効化。食品・包帯・弾薬などfalseのものは標準品質 |
| `resource_costs` | 未対応。空以外はエラーにする。木炭などは通常の材料として消費 |
| `CraftJob.selected_resources` / `ResourceSelection` | 現行未使用。資源消費を未対応としているため選択されない |
| `data/production/recipes.lua` のrules | 全て使用中。`success_base` / `dexterity_step` / `skill_step` / `success_min` / `success_max` / `tool_step` / `tool_cap` / `material_step` / `material_cap` / `quality_thresholds` / `failure_material_percent` |
| `data/progression.lua` | 全て使用中。`experience_per_level` / `practice_base` / `practice_per_level` / `max_skill` / `max_level` / `attribute_chance_percent` / `hp_min` / `hp_max` / `stamina_min` / `stamina_max` / `sanity_min` / `sanity_max` / `combat_skill_step` |

確認先：[制作判定](../src/game/crafting.rs)、[成長](../src/game/progression.rs)。
Scenarioのサイズ・seed・地面・境界・プレイヤー・spawns・labels、テクスチャmanifest/rulesと演出のフレーム登録も読込・生成・表示に使用している。これらは能力の未接続とは別の、実際に使っている配置・表示設定。

`data/mental.lua` の `psychic_power_step` / `sanity_resistance_step` / `ghoul_glyph` は共通精神攻撃・グール化処理で使用する。

## 今回の整理

消毒薬の定義・通常/試験マップの配置・有効PNG・登録・専用Lua検査からの参照を削除。治療は包帯による止血とHP回復を中心とし、傷の汚染・消毒の管理は採用しない。
汎用の状態削除APIは他のLua効果にも使えるため維持する。原画アトラス内の消毒薬の絵は歴史資料として残すが、有効な素材一覧とゲームには登録しない。
体力・知力・社交を削除し、器用さの命中/制作補正と、精神力・スタミナ・正気度の成長/判定を追加した。具体的なサイキック能力と操作、正気度被害の発生源・回復操作は未実装。
