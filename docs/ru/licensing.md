<a id="лицензии-и-атрибуция"></a>

# Лицензии и атрибуция

[English](../licensing.md) | [Русский](licensing.md) | [Документация](README.md)

<a id="лицензия-moduleengine"></a>

## Лицензия ModuleEngine

ModuleEngine распространяется по двойной лицензии, на ваш выбор:

- лицензия MIT ([LICENSE-MIT](../../LICENSE-MIT)) или
- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE)).

Каждый крейт воркспейса объявляет `license = "MIT OR Apache-2.0"` через
`[workspace.package]` и `publish = false`, потому что крейты не публикуются на
crates.io. Версии до коммита `279b036` включительно распространялись только по
MIT; у получателей этих копий эта лицензия сохраняется.

Если вы явно не укажете иное, любой вклад, намеренно отправленный для включения
в ModuleEngine, в смысле лицензии Apache-2.0, лицензируется на тех же двойных
условиях без дополнительных требований.

<a id="third-party-code"></a>

## Сторонний код

[`THIRD-PARTY-LICENSES.txt`](../../THIRD-PARTY-LICENSES.txt) перечисляет каждый
Rust-пакет, который компилируется в бинарники для Windows и Linux, с его
SPDX-выражением лицензии и текстами лицензий и уведомлений, которые пакет
поставляет. Пакеты только для других платформ и dev-зависимости не включаются.
Для пакета без собственного файла лицензии добавляется стандартный текст
предпочтительной лицензии; шрифты egui по умолчанию добавляют уведомления
OFL-1.1, Ubuntu Font Licence, Hack и emoji-шрифта.

[`scripts/third_party_licenses.py`](../../scripts/third_party_licenses.py)
создаёт файл из `Cargo.lock` средствами стандартной библиотеки Python и
`cargo metadata`. Скрипт также проверяет список разрешённых лицензий: MIT,
MIT-0, Apache-2.0 (в том числе с LLVM-exception), Zlib, BSD-2-Clause,
BSD-3-Clause, ISC, BSL-1.0, 0BSD, Unlicense, CC0-1.0, Unicode-3.0,
Unicode-DFS-2016, OFL-1.1 и Ubuntu-font-1.0. Пакет проходит проверку, если хотя
бы одна альтернатива его выражения использует только разрешённые лицензии.
Принимаются только пакеты с crates.io; git- и path-зависимости не проходят
проверку.

После изменения зависимостей или `Cargo.lock` выполните:

```text
cargo fetch --locked
python scripts/third_party_licenses.py
```

CI запускает `python scripts/third_party_licenses.py --check` на обеих
платформах. Проверка падает, если поставляемый пакет использует лицензию вне
списка или закоммиченный файл отличается от заново созданного.

<a id="что-входит-в-поставки"></a>

## Что входит в поставки

| Поставка | `LICENSE-MIT`, `LICENSE-APACHE`, `THIRD-PARTY-LICENSES.txt` | DXC `LICENSE-MS.txt`, `LICENSE-LLVM.txt` |
| --- | --- | --- |
| Full source export | Да, обязательно | Да, вместе с `tools/dxc` |
| Team export | Да, обязательно | Да, когда подключён Vulkan-рендерер |
| Standalone Player | Да, обязательно; `README.txt` ссылается на них | Не поставляются; в Player нет DXC |

Экспорт Player копирует эти три файла из пакета движка, который его собирает, и
завершается ошибкой, если какого-то нет. Они перечислены и захешированы в
`player.json`, как остальные файлы пакета, и ассет проекта не может использовать
эти имена.

<a id="ассеты-в-репозитории"></a>

## Ассеты в репозитории

| Путь | Происхождение | Лицензия |
| --- | --- | --- |
| `assets/audio/*.wav`, `assets/models/demo_crystal.glb` | Созданы для тестовых фикстур ModuleEngine | MIT OR Apache-2.0 |
| `assets/gpu/*.hlsl` | Шейдеры ModuleEngine | MIT OR Apache-2.0 |
| `examples/crate-garden/assets/` | Созданы [`generate_assets.py`](../../examples/crate-garden/generate_assets.py); см. [Crate Garden](../../examples/crate-garden/README.ru.md) | MIT OR Apache-2.0 |
| `crates/editor/assets/editor.ico` | Иконка редактора ModuleEngine | MIT OR Apache-2.0 |
| `examples/bench/khronos/Avocado.glb`, `BoxVertexColors.glb` | [Khronos glTF Sample Assets](https://github.com/KhronosGroup/glTF-Sample-Assets/tree/e124e8d8d7330a5372bc2b4d00d39a4a9d7997ea/Models), без изменений | CC0-1.0; файлы лицензий лежат рядом с моделями |
| `docs/media/*.png` | Снимки экрана редактора и Player ModuleEngine | MIT OR Apache-2.0 |
| `docs/media/blender-workspace-tabs-reference.png` | Снимок панели вкладок Blender 4.3, только как образец интерфейса | Интерфейс и логотип Blender принадлежат Blender Foundation; в игры не поставляется |

<a id="загружаемые-ассеты"></a>

## Загружаемые ассеты

Модель Sponza для бенчмарков распространяется по
[CRYENGINE Limited License Agreement](https://github.com/KhronosGroup/glTF-Sample-Assets/blob/e124e8d8d7330a5372bc2b4d00d39a4a9d7997ea/Models/Sponza/LICENSE.md)
и не перераспространяется. [`manifest.json`](../../examples/bench/sponza/manifest.json)
фиксирует её файлы по SHA-256, а `scripts/bench.py` скачивает их в игнорируемую
Git папку `.tools/bench-assets/sponza/`. См. [бенчмарки](bench.md). Не
коммитьте эти файлы и не включайте их в экспорт.

Бинарники DXC скачиваются и фиксируются в
[`toolchain-manifest.json`](../../toolchain-manifest.json); в `tools/dxc/`
отслеживаются только их файлы лицензий.

<a id="подпись-кода"></a>

## Подпись кода

Бинарники ModuleEngine пока не подписаны: CI проверяет исходники, но не собирает
релизные бинарники, и сертификат подписи не настроен. Windows SmartScreen может
предупреждать при запуске неподписанных `ModuleEngine.exe` или `Player.exe` на
другом компьютере. Для подписи нужен сертификат или аккаунт сервиса подписи,
принадлежащий проекту; она будет добавлена вместе с релизным workflow. Ключ
подписи должен храниться в секретах CI или в аппаратном или облачном HSM и
никогда не должен попадать в репозиторий.
