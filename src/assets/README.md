# 日本語フォント

`japanese16x16.png` はゲームへ埋め込む文字画像です。実行時のフォント追加インストールは不要です。
16×16ピクセルのタイルを32列に並べています。

- 0〜255: bracket-lib の `terminal8x8.png` を2倍に拡大したCP437文字。地形・罫線・英数字との互換性を維持します。著作権・ライセンスは `BRACKET-LICENSE.txt`。
- 256以降: RustソースとLuaデータの非ASCII文字をUnicode順に収録。対応表は `japanese_chars.txt`。ヒラギノ角ゴシックで描画した文字画像で、フォントファイル本体は同梱しません。

UIやLuaに新しい文字を追加した場合は、macOSで次を実行してPNGと対応表を同時に更新してください。
引数のCP437画像には、プロジェクトで導入しているbracket-libの `bracket-terminal/resources/terminal8x8.png` を指定します。

```sh
swift -module-cache-path /private/tmp/rogue-swift-cache assets/generate_font.swift /path/to/terminal8x8.png "$PWD" "$PWD/../data"
cargo test
```

未収録の文字は `?` で表示します。テストでUIソースとLua定義の日本語名の収録を確認します。
内部ログは英語を保持し、通常の操作・戦闘ログは表示時に日本語へ変換します。未知の診断エラーは原文を表示します。
