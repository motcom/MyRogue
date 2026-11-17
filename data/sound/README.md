# 試作効果音

Python標準ライブラリで波形を合成した、短いゲーム用効果音。
外部API・録音素材・追加パッケージは使用していない。
形式は44,100 Hz・モノラル・16bit PCM WAV。

| ファイル | 長さ | 音の構成 |
| --- | --- | --- |
| `melee_attack.wav` | 0.30秒 | 振る風切り音と短い打撃音 |
| `gunshot.wav` | 0.44秒 | 鋭い破裂音・低音・小さな機械音。銃の共通試作音 |
| `crossbow.wav` | 0.38秒 | 弦の振動・木のクリック・ボルトの風切り音 |
| `reload.wav` | 0.68秒 | 弾倉の取り外し・挿入、スライド/ボルト操作のクリック・摩擦音 |
| `throw.wav` | 0.28秒 | 手から離れる短い音と風切り音 |
| `explosion.wav` | 0.95秒 | 破裂音、低い衝撃、減衰する轟音 |
| `flashbang.wav` | 0.62秒 | 鋭い破裂音と短く控えめな金属的な余韻 |

再生成（作業ディレクトリに依存せず、既存の同名7ファイルを更新する）:

```sh
python3 tools/generate_sounds.py
```

別の場所へ生成:

```sh
python3 tools/generate_sounds.py --output-dir /private/tmp/my-rogue-sound-preview
```

macOSでの試聴例:

```sh
afplay data/sound/melee_attack.wav
afplay data/sound/gunshot.wav
afplay data/sound/crossbow.wav
afplay data/sound/reload.wav
afplay data/sound/throw.wav
afplay data/sound/explosion.wav
afplay data/sound/flashbang.wav
```

乱数シードを固定しているため、同じPython環境で再生成したWAVは一致する。
音色・長さは生成スクリプトで調整する。ゲームの再生は `src/media/sound.rs` のrodio実装へ接続済み。
ゲーム内の共通音量は0.45、同時再生は最大16音。`--mute` で無音にできる。
起動時に7ファイルを読み込み、音声デバイスまたはファイルが利用できない場合は理由を標準エラーへ出して無音で継続する。

装填が成功したときだけ一度リロード音を再生する。銃・クロスボウ共通の試作音で、プレイヤーと視野内の敵に適用する。満装填・弾薬なし等の失敗時と視野外の敵では鳴らない。音による追加ターン消費はない。実機での試聴は未確認。

投擲音は表示キューの飛翔開始時、爆発音と閃光音は着弾時に各一度だけ再生する。前の射撃・投擲の演出が終わるまで次の音も待つ。飛翔・効果範囲とも視野外なら演出と音を発行しない。失敗・中止した投擲は無音。新しい3音の実機試聴は未確認。
