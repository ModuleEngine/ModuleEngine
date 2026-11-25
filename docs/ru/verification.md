# Маршруты проверок

[English](../verification.md) | [Русский](verification.md) | [Документация](README.md)

Реестр [scripts/verification.json](../../scripts/verification.json) и stdlib launcher
[scripts/verify.py](../../scripts/verify.py) выбирают существующие проверки. Они не
меняют Rust, Cargo profiles, продуктовые watchdogs или критерии производительности.
Новый маршрут не заменяет полную приёмку из исторических отчётов.

[Диагностика редактора](editor-diagnostics.md) — отдельный ограниченный маршрут с
предоставленным обычным executable, сгенерированными fixtures и проверенными native
captures; он не собирает Editor.

<a id="запуск-и-окружение"></a>

## Запуск и окружение

Скриптам нужен Python версии 3.9.17 или новее с `tarfile.data_filter`. Они
используют стандартную библиотеку. В CPython 3.9.17 перенесли фильтр
распаковки, который используется при восстановлении DXC. Ранним патч-версиям
Python 3.10 и 3.11 тоже нужен этот бэкпорт. CI использует Python 3.12. [Список изменений CPython 3.9.17](https://docs.python.org/release/3.9.17/whatsnew/changelog.html#library)

Для профилей проверки также требуется настроенная нативная цепочка Rust/Cargo.
Launcher ничего не устанавливает, а Cargo вызывается
с `--offline --locked`. Проверку закреплённых инструментов выполняйте существующим
`scripts/bootstrap.ps1 -Json`; восстановление остаётся отдельной явной операцией.
В Linux подключите существующий `.tools/linux-x86_64/env.sh` и сверяйте условия в
[Linux guide](linux-support.md). Launcher наследует toolchain и normal loader
environment, включая законный DXC `LD_LIBRARY_PATH`; он не настраивает их заново.
Windows wrapper новых профилей также не подменяет `CARGO_HOME`: если готовый
offline registry находится в `.work/cargo-home`, до запуска задайте
`$env:CARGO_HOME = (Resolve-Path .work/cargo-home).Path`. Используйте фактический
установленный cache; отсутствие пакетов диагностируется Cargo, не исправляется
загрузкой или очисткой. Тесты, которые собирают воркеры Custom SDK или читают
исходники их офлайн-зависимостей, проверяют выбранный кеш Cargo один раз на
тестовый процесс. При отсутствии зависимостей каждый такой тест сообщает
причину и команду подготовки `cargo fetch --locked`. В CI зависимости
обязательны, и пропуск теста SDK из-за кеша проваливает проверку.

```powershell
$python = 'C:\path\to\python.exe'
.\verify.ps1 -List -Python $python
.\verify.ps1 -Profile contracts -Plan -Python $python
.\verify.ps1 -Profile quick -Python $python
```

`-Python` имеет приоритет над `MODULE_ENGINE_VERIFY_PYTHON`. Затем wrapper проверяет
доступный Python/py. WindowsApps aliases не запускаются; персональный путь Python
не хранится в репозитории. `-Plan`/`-List` не запускают Cargo/bootstrap и не создают
run-каталоги. При прямом запуске Python используется тот же CLI:

```sh
python3 scripts/verify.py --list
python3 scripts/verify.py --profile native --plan
./verify.sh --profile quick
```

`MODULE_ENGINE_VERIFY_PYTHON` может задавать interpreter и для shell wrapper.
У прямого `verify.py` профиль по умолчанию `quick`; **у старых wrappers без
аргументов остаётся прежний тяжёлый legacy suite**.

## Профили

| Профиль | Объём | Чего он не подтверждает |
| --- | --- | --- |
| `quick` | `cargo fmt --all -- --check`, все lib tests пакетов `module-engine-core` и `module-engine-project`. | Не workspace, Editor, Vulkan runtime, secondary Custom builds или ignored acceptance. Холодная компиляция всё равно может быть долгой. |
| `contracts` | Quick плюс `cargo_build::tests::`, четыре Inspector-регрессии, 99 сценариев свойств Inspector, четыре прежних Save As tests, 28 сценариев сохранности workflow и два теста `diagnostic_personal` для проверки явного root и изоляции Recent/layout с сохранением source permissions. | Не всё UI, native dialogs или полный authoring suite. |
| `clippy` | Строгий Clippy для всех целей и features workspace, затем для обычного исполняемого файла Editor в Release. Оба шага используют `-D warnings` и закреплённый нативный toolchain. | Без formatting, tests или runtime-проверок; не приёмка движка или поставки. |
| `render` | CPU-проверки графа и кеша, компиляция HLSL через DXC и offscreen-регрессии Vulkan, включая обновления сцены GameHost, отклонение тика и смену сцены. | Без окна, доставки и проверки производительности. |
| `native` | Пять явных ignored tests: Stop/reuse, slow-UI/hot-apply, native linker, cache context и Windows executable spellings. | Не desktop benchmark, screen latency, Full export или строгие 60 FPS. Windows-only test на Linux отмечается `skipped`, а не пройденным. |
| `native-tools` | Три native linker/cache/executable-spelling проверки из `native`, без GameUI и GPU. | Не полный `native`; на Linux Windows-only test отмечается `skipped`. Используется Windows CI. |
| `package` | Две ignored проверки `game_package`: native target modes и concurrent immutable snapshots. Они собирают минимальные Cargo fixtures и исполняют их продукты. | **Не реальный Full/Team/Player game export, relocation или полная приёмка поставки.** Эти CLI-проверки остаются адресным/legacy маршрутом. |

Профиль `clippy` явный и независимый: `contracts` по-прежнему проверяет formatting
ровно один раз; он не добавляет шагов в legacy suite. Сначала выполните
`python scripts/verify.py --profile clippy --plan`, затем
`python scripts/verify.py --profile clippy`. Планирование не запускает процессы
и оставляет `{native_host}` неразрешённым. Исполнение сверяет фактические release
и native host из `rustc -vV` с `toolchain-manifest.json` на Windows или
`scripts/linux-toolchain.json` на Linux до Cargo metadata; несовпадение — отказ.

`module-engine-vulkan` запрещает `clippy::undocumented_unsafe_blocks` для всех
целей сборки. То же правило действует в модулях нативного вывода, диалогов
Windows и ожиданий Host. Комментарии безопасности объясняют требования к
времени жизни, владению и синхронизации в месте вызова.

Команды проверок Editor и нативный захват входят в сборку с
`--features verification`. Launcher выбирает эту возможность для тестов
Editor, а обе legacy-обёртки включают её для своих команд Editor. Обычные
пакеты Editor собираются без неё. Отдельный маршрут нативных бенчмарков
S1–S5 по-прежнему включает `bench`.

Интеграционный тест `verification_cli` запускает настоящий Editor. CI
выполняет его с `verification` в наборе workspace и отдельно в обычной
Release-сборке. В первом случае `--self-check` должен записать успешный
отчёт проверки на CPU. Обычный Editor должен завершиться с сообщением о
требуемом feature и оставить каталог отчёта отсутствующим.

25 ноября 2026 года две Release-сборки Windows x64 на Rust 1.99.0 заняли
37 123 584 байта со стандартными features и 38 485 504 байта с
`verification`. Разница — 1 361 920 байт (3,54% от сборки с feature).
У обеих один дайджест рабочего снимка исходников
`ec5bc2b26b2a8e620d4bc684e75c7a2ac1641dd3dd75e13fbd200fd895f530ec`.

Проверки выполняются последовательно. Для Rust tests launcher сначала получает
`cargo metadata`, затем `cargo test --no-run --message-format=json` для выбранного
package/target. `contracts` использует обычную test-сборку; `native` и `package` -
release. Фактический native host задаётся явно через `--target`; чужой явный
`CARGO_BUILD_TARGET` отклоняется. Target directory берётся из Cargo metadata,
не собирается предположением `target/debug` или `target/release`.

Прямой test executable выбирается по package ID, canonical manifest, target
name/kind и `profile.test=true`. Нужны успешный Cargo process, ровно один
`build-finished.success=true`, один distinct executable, существующий native
PE/ELF внутри target directory. `fresh=true` допустим. Путь и SHA записываются;
подмена файла между шагами отклоняется.

Test binary запускается **напрямую в normal loader environment**, а не через
`cargo test` или `cargo run`. Для exact tests сначала требуется единственный
результат `--list --ignored --exact <name>`; ноль тестов никогда не считается
успехом. После выполнения сверяется libtest summary с найденным набором. Это
сохраняет важное условие Stop/reuse из [предыдущей проверки loader](postfix-p2-fixes-20261016.md).

<a id="vulkan-dxc-capabilities"></a>

## Возможности Vulkan и DXC

Профиль `render` проверяет контракты зависимостей шейдеров и графа на CPU,
компилирует HLSL в тестах с DXC и выполняет Vulkan-тесты на совместимом
устройстве Vulkan 1.3. Адаптер должен поддерживать dynamic rendering и очередь
graphics+compute. Для offscreen-проверок подходит и программное устройство Vulkan.
Тестам, которые сами готовят запечатанные шейдеры Player, нужен DXC на этапе
подготовки. Тест отката выделений требует Vulkan и DXC для повторного рендера.
Для тестов компиляции и
отражения шейдеров достаточно DXC. Обоим сценариям GameUI профиля `native`
нужны Vulkan и DXC. Проверки линкера, контекста кеша и имён Windows EXE
выполняются независимо.

Предварительная проверка использует нативную инициализацию рендерера, его
фактический поиск компилятора и `--version`. При отсутствии возможности
пишутся имя теста и причина. Если пропущены все тесты шага, его статус —
`skipped`. В смешанном шаге `test_skips` содержит пропуски, а `passed_count`
считает выполненные успешные тесты. Некорректно настроенный компилятор,
отклонение шейдера, ошибка Vulkan API после успешной предварительной проверки,
сбой libtest и ошибки валидации приводят к отказу. Флаг `--require-capabilities`
(`-RequireCapabilities` в PowerShell) делает пропуск по возможности ошибкой.
Linux CI использует его для offscreen-маршрута. Пропуски по платформе сохраняют
прежнее поведение. При последовательной проверке Editor дожидается завершения
своих фоновых задач и освобождения ресурсов GPU до проверки итогов валидации.
Счётчик ошибок валидации общий для процесса. При параллельном запуске GPU-тестов
ошибка может быть отнесена к соседнему тесту. Локальные Linux-прогоны выполнены
на Mesa в WSL. Нативный Linux-раннер проверяется в CI pull request.

Вызов wrappers без аргументов по-прежнему выполняет legacy suite.
Windows-маршрут вызывает `cargo.exe` напрямую и передаёт разделитель аргументов
`--` в Clippy и `cargo run`. Оба скрипта выводят пропуски в консоль, в том числе
для зависимых CLI-проверок Windows. CPU-проверки
продолжают работу. Опциональному оконному бенчмарку Windows нужны его реальные
возможности окружения. Восстановление инструментов остаётся явной операцией.

<a id="isolated-source-fixtures"></a>

## Изолированные установки исходников в тестах

Тесты workflow и персональных раскладок создают собственную временную установку
исходников, копируют реальные файлы из обязательного native inventory и записывают
manifest через `write_full_manifest`. Обычное определение distribution и binding
проекта должны выбрать эту установку и разрешить Full authoring до запуска сценария.
Проект workflow и его установка Engine находятся в соседних каталогах, поэтому
Save As копирует только проект. Каталог удаляет создавший его тест.

Эти тесты файловых команд не компилируют игру или Custom worker. Native и package
проверки продолжают использовать свои реальные исходники и toolchain-контексты.
Тесты не обновляют metadata пакета в checkout; устаревший или неполный inventory
по-прежнему отклоняется production-проверкой исходников. Так посторонняя metadata
checkout не скрывает проверки Save As, истории, Recent и персональных настроек в CI.

## Время сборки и тестов

[Производительность сборки/проверок](build-verification-performance.md) разделяет Cargo
compile/link, прямые тесты и затраты launcher/observer. Локальные target-cold/repeat
отчёты не доказывают hosted cache выигрыш или cold-start Editor.

## CI

[GitHub Actions и обязательный check](ci.md) выполняют отдельные планы/проверки `contracts` и `clippy` на Windows и
Linux, включая документационные PR, и добавляют native/package маршруты для изменений исходников. Перед offline
проверками CI явно восстанавливает pinned Rust с `rustfmt`/`clippy` и locked dependencies. Для
обязательности merge требуется применённое правило ветки; наличие YAML не является
доказательством первого успешного hosted запуска или настроенной защиты.

## Проверки свойств Inspector

Три `game_ui::tests::inspector_generated_*` в
[game_ui.rs](../../crates/editor/src/game_ui.rs) входят в `contracts` как отдельные
exact tests. Это конечная воспроизводимая матрица без новой зависимости:
шесть ключей (`literal.key`, `0`, пустой, `/~`, `a/b`, `a~b`) и восемь payload
(null, bool, число, строка, пустые object/array, вложенный object и mixed array).

- 48 случаев проверяют, что Apply без редактирования сохраняет authored документ
  и не переименовывает существующий local opaque member после save/reopen.
- 48 случаев проверяют отсутствие local delta для унаследованных данных и
  получение будущих изменений prefab/defaulted fields после save/reopen.
- Три положительных контроля проверяют настоящий registered edit, сохранение
  существующего override и намеренную локальную правку opaque aggregate.

Используются настоящий egui Apply, production save/read и проверка контрактов
документа. Ожидания — исходный authored документ и явно заданные последующие
значения; тесты не воспроизводят алгоритм flatten/merge/ownership. Проверка
`get(key)` различает присутствующий null и отсутствующий member; inactive unknown
payload и соседний namespace `foo.bar` должны сохраниться. Положительный edit
контроль не позволяет принять бездействующий Apply за успешную проверку.

Матрица не является случайным fuzzing и не доказывает все возможные JSON или
операции Assets. При новом дефекте добавляйте его минимальный случай и расширяйте
соответствующее свойство, сохраняя существующие проверки typed references/defaults.

## Сохранность Save As и файловой истории

Три exact tests `app::editor_workflow::workflow_tests::preservation_*` в
[editor_workflow.rs](../../crates/editor/src/editor_workflow.rs) используют настоящие
команды, worker, save/read и файловую историю. Матрица конечная, без fuzzing и
новых зависимостей:

- 18 случаев: шесть точных ключей/значений и бинарных payload через Cancel,
  занятый ресурс назначения и успешный Save As. Проверяются исходные байты,
  dirty JSON, Play/history/Recent, opaque payload, stable asset ID и Engine binding.
- Четыре случая Rename/Delete × Undo/Redo: отказ публикации JSON обязан откатить
  файлы и сохранить историю. Единственный допустимый staged recovery JSON
  сверяется с кандидатом; повтор после восстановления пути обязан пройти.
- Шесть случаев ветвления истории: Rename → Undo → Delete не позволяет вернуть
  отброшенную ветку через Redo. Сохраняются стабильные ID, unknown/null payload,
  бинарные байты, соседний ресурс и исходный binding.

Ожидания — оригинальный документ, сохранённые bytes/hash и strict reopen,
а не повторение алгоритма rollback. Успешный Save As служит положительным контролем.
Четыре существующих `workflow_tests::save_as_*` также входят в `contracts`:
picker error, same-path publication failure, copy и invalid JSON.

Это не crash/power-loss recovery, все возможные I/O отказы или GUI dialogs.
Конфликт ресурса проверяет отказ до публикации, не откат её частичного завершения.
Реальный отказ записи отдельно проверяется в Undo/Redo. На WSL/DrvFS Full fixtures
медленнее; потолок двух больших матриц — 300 секунд, без изменения product watchdogs.

## Выбор по изменению

| Что меняется | Начальный маршрут | Дополнительное доказательство |
| --- | --- | --- |
| Только нейтральный Core или Project data/history | `quick` | Адресная регрессия изменённого контракта. |
| Inspector, schemas/defaults, prefab, authored JSON | `contracts` | Для нового UI-поведения — его отдельный сценарий; четыре прежние регрессии не проверяют все окна. |
| Save As, Assets и файловая Undo/Redo history | `contracts` | Адресный сценарий нового отказа; bounded матрицы не заменяют остальные export/authoring контракты. |
| Custom compiler, toolchain, store, lifecycle/slow ticks | `contracts`, затем `native` | Измерения без конкурирующих native/GPU/build операций; desktop/cold-start проверяются отдельно при изменении этих требований. |
| Выбор Cargo products или Player packaging | `contracts`, затем `package` | Для изменённого export/delivery — отдельная Full/Team/Player приёмка реальной игры. |

Это начальный набор, а не разрешение пропустить проверку нового поведения. Для
исправления сначала воспроизведите ошибку; после правки повторите его сценарий
и затронутые контракты. Повторять весь legacy suite после документационной правки
или успешного малого тестового изменения без нового основания не требуется.

## Инвентаризация тестов ignored

Все 23 теста с `#[ignore]` перечислены в реестре с назначением. Семь включены в
`native`/`package`. Для остальных 16 в [реестре](../../scripts/verification.json)
указаны условия отдельного запуска. Некоторые из них:

- `runtime_delivery::tests::emit_stage4_software_inactive_vulkan_fixture` - только
  подготовка fixture; требует нового абсолютного `MODULE_ENGINE_STAGE4_FIXTURE_PATH`.
- `compat_tests::audit_slow_custom_catchup_wall_time` - историческая диагностика;
  не gate нового поведения или производительности.
- `app::panes::tests::g1_team_registered_palette_pointer_apply_history_save_preserves_frozen_execution`
  требует настоящей экспортированной Team-поставки в `MODULE_ENGINE_TEST_TEAM_ROOT`.
- Четыре нативные проверки `audio::tests` в `module-engine-host-services` требуют
  feature `audio` и настоящего аудиоустройства Windows. Они проверяют скорость
  подачи звука, зарегистрированное воспроизведение PCM8, повтор запуска после ошибки
  и освобождение независимых владельцев. Запускайте их отдельно и последовательно
  по [руководству Host Services](../../crates/host-services/README.ru.md#одновременные-wav-музыка-и-эффекты).
- `tests::diagnostic_long_attempt_and_idle_cpu_cost` в `module-engine-g1` —
  добровольно запускаемая release-диагностика времени CPU без конкурирующей нагрузки.
  Её результаты не подтверждают производительность отрисованных кадров,
  нативного звука или операций с хранилищем.
- Две [сквозные проверки R1.2](ecs-world.md#r12-verification) запускаются
  последовательно из checkout с полными исходниками. Они компилируют Rust-воркеры
  для повторного сохранения мигрированной игры и экспорта самостоятельного Player.

Эти шесть проверок G1 сохраняют отдельные результаты выполнения и границы в
[отчёте о поставке G1](g1-delivery-20261027.md) и
[продолжении проверки](g1-verification-20261101.md).
Регистрация не добавляет их в автоматический профиль и не означает нового успешного запуска.

Registry не даёт произвольного `--step`, который незаметно включал бы emitter.
Для каждого нового теста с `#[ignore]` укажите назначение и выберите профиль или явное исключение из профилей.

## Сохранность, сериализация и результат

Каждый запуск создаёт новый `outputs/verification/<UTC-time>-<nonce>/` с
`result.json` и отдельными stdout/stderr logs каждой команды. Launcher-owned
TEMP/TMP/TMPDIR указывают на уникальный короткий `.work/v/<run-token>` только в дочернем
окружении. Существующие tests могут создавать собственные изолированные fixtures
в `.work/test-temp` и использовать caches согласно своему контракту. Короткий TEMP
уменьшает вложенные MSVC object paths; resolved путь обязан оставаться в checkout,
фактический `temporary_directory` записывается в отчёт. `--output-root` допускается только внутри `outputs`
этого checkout; существующие отчёты не перезаписываются.

Один OS file lock `work/verification/launcher.lock` сериализует все исполнения
нового launcher в этом checkout: в том числе shared GPU/native setup и
конкурирующие build steps. При занятом lock второй запуск завершается с явной
ошибкой. Lock освобождается ОС при закрытии процесса; stale-файл удалять не нужно.
Этот lock **не блокирует legacy wrappers, внешние программы или вручную запущенный
Cargo**. Native/GPU/timing сценарии по-прежнему запускайте без таких конкурентов.

Перед и после шагов проверяется inventory корневых JSON, `assets`, `game-assets`,
`.user`, `.history`, `.asset-history`, `.asset-trash`. Новые/изменённые/удалённые
защищённые файлы означают failure. Launcher не делает reset, очистку или
автоматический restore пользователя. Это контроль изменения, не filesystem
sandbox; внешние связанные ресурсы за пределами этих каталогов им не защищаются.
Сами выбранные tests используют собственные fixtures, не авторский game JSON.

Report фиксирует UTC timestamps, profile/scope, SHA реестра/launcher/wrappers, доступный HEAD и
digest tracked dirty diff, native host/toolchain, команды/cwd/exit codes/timeouts,
логи, выбранные executable paths/SHA и test names. Если Git недоступен, записывается
ошибка snapshot; untracked source не входит в dirty diff digest. Защищённый author
inventory фиксируется отдельно. Это не новая сертификация published EXE.

Статусы `passed`, `failed`, `skipped`, `not_run` различаются. После ошибки следующие
шаги не выполняются; platform skip содержит причину. Ошибка preflight оставляет
шаги `not_run`. Ненулевой exit или отсутствующая программа не могут унаследовать
успех прежнего процесса. На timeout завершается только запущенное launcher дерево
процесса; результат cleanup записывается. Windows использует `taskkill /PID /T`,
Linux - собственную process group; это не управление посторонними процессами.
`--timeout <seconds>` - явный предел каждой команды, включая компиляцию; он не
меняет watchdog внутри продукта. По умолчанию build timeout 1200 секунд, test
timeouts указаны в реестре.

## Legacy и performance

`verify.ps1` и `verify.sh` без аргументов сохраняют прежний набор проверок и
проверяют Python до тестов. В `verify.ps1 -NativeBenchmarks` та же проверка
выполняется через bootstrap. Обычный release Editor выбирается из CargoJSON с проверкой
identity/unique successful product и native PE вместо ручного пути.

Отдельного нового `performance` profile нет. Legacy `-NativeBenchmarks` запускает
весь прежний suite, затем native Editor/Player benchmarks на выбранном legacy
проекте; его исторические outputs повторно используются. Этот legacy маршрут не
получает новые author-file guards, timestamp reports и lock автоматически. Editor
benchmark требует isolated Full: при создании Editor возможна запись Recent.
Для строгой приёмки используйте отдельный проверенный Full package/окружение и
адресные normal executable команды из соответствующего отчёта; не запускайте
legacy benchmark на единственном авторском проекте.

Один exit 0 legacy script не подтверждает Player `target_met=true`: нужно читать
`native-frame-report.json`. Сохраняются различия functional completion,
`measurements_valid`, cold/prewarmed, hardware/software Vulkan и strict cadence.
Новые native/package profiles также не закрывают открытые цели из
[current-status](current-status.md).

## Проверка launcher без Cargo и GPU

```powershell
& $python -B -m unittest discover -s scripts -p test_verify.py -v
```

Fixtures создаются только в `work/verification-tests`. Тесты используют synthetic
CargoJSON/PE/ELF, mocked test processes и собственные короткие Python children
для exit/timeout; Rust/Cargo/native engine не запускаются. Они проверяют profile
isolation, все пятнадцать ignored entries, OS skip, selector failures, zero-test
rejection, normal environment, lock, отчёт отказа и сохранность author files.
Это unit проверка launcher; реальные профили должны проходить отдельно на
целевом Windows/Linux окружении и текущем source snapshot.

## Проверка внедрения 16 октября 2026

Работа выполнена поверх `c64bc34ffd28419cfd851838c16a2a48c9c2fcf8` с сохранением
исходных пользовательских изменений. Этапы проверялись последовательно:

| Проверка | Результат |
| --- | --- |
| Контекст проекта | Уточнены актуальные контракты и открытые цели; проверены ссылки и UTF-8/LF. |
| Launcher и wrappers | 18 Python unit tests; отдельные dispatch/legacy/shell fixtures и 20 mocked CargoJSON/PE cases. |
| Windows, этап маршрута | `quick`, `contracts`, `native` (5 tests), `package` (2 tests) прошли. |
| Linux Ubuntu/WSL, этап маршрута | `quick` прошёл: 102 Core + 59 Project tests. |
| Windows после новых свойств | `contracts` прошёл: 102 Core + 60 Project + 9 Cargo + 7 Inspector = 178 tests. Три Inspector tests исполнили 99 новых сценариев. |
| Linux после новых свойств | `contracts` прошёл: 102 Core + 59 Project + 10 Cargo + 7 Inspector = 178 tests, включая ту же матрицу. Набор platform-specific tests отличается от Windows. |
| Чувствительность к дефекту | Временный возврат dot-path записи вместо точного member дал ожидаемый отказ на `key-0-null`; production-код восстановлен побайтно, затем Windows/Linux contracts прошли. |
| Пользовательская работа | Все 11 исходно защищённых файлов сохранили SHA-256, включая `game-project-v4.json`, служебные данные и четыре прежних EXE. |

Команды, фактические test executable/SHA и логи лежат в отдельных
`outputs/verification/<run-id>/result.json`; индексы прогонов и контрольные суммы
собраны в [результатах внедрения](../../outputs/development-rollout-20261016/acceptance.json).
Rust diff этого этапа добавляет только тесты; Cargo dependencies не менялись.

Первый ручной Windows baseline без настройки TEMP получил отказы доступа;
повтор с локальным TEMP прошёл. Новые профили используют собственный TEMP.
Git Bash mock harness также повторён вне ограниченной песочницы после отказа
MSYS создать служебный объект. Это сохранённые отказы окружения, а не замолчанные
успехи. В WSL файловые tests на `/mnt/e` заметно медленнее native filesystem;
их время не является измерением движка.

Полные legacy suites без аргументов, desktop/performance benchmarks и реальная
Full/Team/Player delivery acceptance в этом внедрении не повторялись. Linux
`native`/`package` не прогонялись новым launcher. Открытые cold-start/60 FPS и
аппаратные Linux цели не закрыты этими проверками.
