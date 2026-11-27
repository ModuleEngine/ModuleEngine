# CI ModuleEngine

[English](../ci.md) | [Русский](ci.md) | [Документация](README.md)

Обычные PR запускают тесты workspace, минимальную конфигурацию Player, проверку
командной строки обычного Release Editor и отдельные проверки
`contracts` и `clippy` на Windows и Linux.
Изменение исполняемых исходников добавляет проверки native и package.
Для PR только с документацией предусмотрена отдельная проверка. `CI required`
успешен после планирования и всех обязательных заданий выбранного маршрута.
Производительность, диалоги рабочего стола, аппаратный GPU и полная поставка
Full/Team/Player находятся за пределами этого CI.

## Что выполняется

[Workflow](../../.github/workflows/ci.yml) запускается для всех PR, push в `main`, merge queue
и вручную; workflow-level path filters отсутствуют. [ci_plan.py](../../scripts/ci_plan.py)
читает Git diff, включая удалённые файлы и обе стороны переименований. Непустой PR проходит
`Documentation checks`, если все изменения затронули обычные неисполняемые файлы
Markdown (`.md` или `.markdown`) в `docs/`,
корневой `AGENTS.md` или README в корне либо непосредственно
в каталоге отдельного crate или примера (`README.md` или `README.ru.md`).
Задание запускает тесты генератора и модели Node,
проверку [согласованности карт EN/RU](roadmap-map.md) и сборку обеих страниц.
Отдельный workflow Roadmap map сохраняет проверку Chromium и публикацию в Pages.

В планировании обоих маршрутов [check_doc_links.py](../../scripts/check_doc_links.py)
сравнивает цели ссылок из документации с базовым коммитом события. Проверка
отклоняет новые ссылки в каталоги `outputs/` и `work/` checkout, а также на
абсолютные локальные пути. Для исторических ссылок сохраняется прежнее число
вхождений в каждом документе. Компактные свидетельства приёмки храните в Git,
как [свидетельства R2.4](evidence/r24/README.md).

Обновление PR отменяет устаревший активный прогон и вытесняет ожидающий прогон
в группе этого PR. У каждого push в `main`, события merge queue и ручного
запуска своя группа по ID прогона. Второе слияние может запустить проверку до
окончания первой и не может отменить или вытеснить её.

Остальные PR проходят матрицу обеих ОС с проверками native и package.
При push и в merge group
лёгкое задание доступно только для согласованных изменений ячеек статуса в обеих картах.
В обычной матрице для событий вне PR правки только в `docs/`,
Markdown, `LICENSE-MIT`, `LICENSE-APACHE` или `.gitignore` пропускают native и
package. Неизвестные исходные пути, пустой diff, первый push и ручной запуск
включают эти маршруты. Ошибка определения diff завершает планирование ошибкой.

| Runner | Обычная матрица | При изменении исходников |
| --- | --- | --- |
| `windows-2025` | Workspace, минимальный Player, Python unit checks, [проверка лицензий](licensing.md#third-party-code), `contracts`, `clippy` | `native-tools` и `package` |
| `ubuntu-24.04` | Workspace на Mesa lavapipe с закреплённым DXC, точечная проверка без DXC, минимальный Player, Python unit checks, [проверка лицензий](licensing.md#third-party-code), `contracts`, `clippy` | `native` на Mesa lavapipe и `package` |

Каждое задание Rust загружает закреплённые зависимости. Задание workspace
выполняет обычные тесты через закреплённый cargo-nextest, а doc-тесты через Cargo.
Результаты JUnit и doc-тестов объединяются по пакетам с указанием причин
пропуска тестов из-за отсутствующей возможности. Сбой обычного теста или отсутствие кеша
Custom SDK проваливает задание. На Linux тесты миграции и восстановления Vulkan
после сбоя выделения, а также сценарии редактора для первой подготовки Scene и
восстановления и применения Rust-кода при скрытой Game должны пройти в workspace
без пропуска. Тест Scene использует нативный рендерер без компиляции игровой
логики. В тесте Rust у каждого из трёх запросов компиляции свой дедлайн. Перед
этим точечная проверка Vulkan выполняется на Mesa без DXC и требует ровно один
тест с явной причиной пропуска из-за отсутствия DXC.

Минимальный Player собирается и тестируется отдельными вызовами Cargo с
`-p module-engine-player --no-default-features`. В графе обычных зависимостей и
зависимостей сборки нет игровых пакетов и Vulkan. Two Arenas используется в
тестовой fixture Player вне этого графа.

Обычная матрица отдельно планирует и выполняет `clippy` на обеих платформах.
На закреплённой
нативной цели она выполняет `--workspace --all-targets --all-features`, затем
`--release -p module-engine-editor --bins` со стандартными features. Обе
офлайн-проверки с закреплёнными зависимостями используют `-D warnings`.
Тесты workspace включают `verification` и запускают настоящий `--self-check`.
Отдельный вызов `cargo test --offline --locked --release -p module-engine-editor --test verification_cli`
проверяет сообщение о требуемом feature в обычной сборке.
Launcher определяет `{native_host}` по фактическому verbose выводу Rust toolchain
и отклоняет несовпадение release/target с manifest платформы до Cargo metadata.
Clippy не выполняет formatting, tests или runtime-сценарии; `contracts` по-прежнему
проверяет formatting ровно один раз. Изменение workflow требует новых hosted
результатов на его SHA.

`native-tools` — три существующие проверки native linker/cache/executable
spellings без GPU или UI. Это часть `native`, а не замена его двух GameUI tests.
Linux full `native` запускает offscreen tests с software Vulkan, без desktop
сессии. Windows GPU native acceptance остаётся адресным локальным маршрутом.
Успешный CI не подтверждает hardware Linux, строгие 60 FPS, cold-start или
интерактивную desktop приёмку. `package` использует минимальные Cargo fixtures,
не настоящий game export. Linux CI передаёт `--require-capabilities`, чтобы отсутствие Vulkan/DXC провалило обязательную offscreen-проверку.

На каждой ОС отдельно запускаются workspace, сборка и проверка CLI обычного Release
Editor, Clippy, contracts и минимальный Player. План может отдельно включить native
и package. У каждого задания своя hosted VM и TEMP. Внутри задания тесты GPU и
native выполняются последовательно. В Windows Cargo собирает продукты на Dev Drive.
Сборки workspace используют rust-lld в Windows и lld в Linux. Вызовы Release Editor,
Clippy и минимального Player используют быстрый линкер и `CARGO_PROFILE_DEV_DEBUG=0`.
Проверки native и Custom SDK сохраняют проверяемое окружение компилятора.
Rust version/native target берутся из существующих Windows/Linux manifests.
Перед offline launcher CI явно устанавливает pinned Rust с компонентами `rustfmt` и `clippy` и делает
`cargo fetch --locked`. Linux-матрица устанавливает системные библиотеки для
линковки и Mesa, затем восстанавливает DXC по хешам архива и файлов из
`scripts/linux-toolchain.json`.
Единственная запись в `LD_LIBRARY_PATH` для тестов указывает на каталог
библиотек закреплённого DXC. Так проверки переиспользования Custom SDK могут
проверить его содержимое.
Сбой установки, fetch, компиляции или теста — отказ CI, без fallback версии.

Launcher использует короткий checkout-local `.work/v/<run-token>` и записывает
его resolved путь в отчёт. Первый Windows hosted run обнаружил LNK1181 в nested
snapshot fixture на длинном TEMP; тест и его критерии сохранены. Точное имя
проблемного object скрыто Cargo, поэтому причина path length — проверяемая гипотеза,
а подтверждение исправления требует нового успешного `package` на hosted Windows.

Отчёты и command logs из `outputs/verification` публикуются как artifact для
каждой платформы даже после отказа; если проверка не стартовала, GitHub job log
содержит ошибку подготовки. Retention 14 дней. Full source/artifact hashes и
scope остаются в обычном `result.json` launcher. У заданий каждой ОС общий кеш
зависимостей. Имя артефакта включает задание, ОС и номер попытки.
[Замеры и границы cache](build-verification-performance.md) определяют
совместимые keys и оценку полного hosted времени, включая стоимость restore/save.

## Включение обязательности

Файл workflow сам по себе не меняет GitHub branch protection. После появления
workflow в целевом GitHub repository и первого успешного PR/push запуска:

1. В ruleset/protected branch включить PR перед слиянием.
2. Добавить required status check **`CI required`**, source GitHub Actions.
3. Проверить отказ теста в отдельном PR: failed prerequisite обязан блокировать
   `CI required`. Новая версия PR должна пройти повторно; старый SHA не подходит.

Итоговый шлюз с `if: always()` отвергает любой результат обязательного задания,
кроме `success`, и отсутствующие данные о маршруте. От невыбранного задания он
ожидает `skipped`. Обычная матрица требует успеха обеих ОС, включая выбранные
шаги native/package. Если branch protection/ruleset ещё не
применён, CI даёт сигнал, но технически не запрещает merge. Включение правила
требует известного repository и соответствующих прав; локальные unit checks
не доказывают выполнение GitHub Actions.

Токен workflow имеет только `contents: read`; checkout не сохраняет credentials.
Нет secrets, `pull_request_target`, привилегированного PR исполнения или
неподготовленного self-hosted job. Все внешние actions закреплены full commit SHA.
Загрузка fresh CI tools выполняется только внутри явно запущенного hosted job.

Для [myjname/ModuleEngine](https://github.com/myjname/ModuleEngine) включён public
доступ по решению владельца. На `main` применены required PR, актуальная ветка,
**`CI required` с GitHub Actions app ID 15368**, защита администратора и завершение
review-обсуждений; force push и удаление ветки запрещены. Required human approvals = 0:
отдельное независимое AI-ревью не выдаётся за approval другого GitHub-аккаунта.
В [служебном PR #2](https://github.com/myjname/ModuleEngine/pull/2) неверный base SHA
дал failure планирования, skipped matrix и failed aggregate; GitHub показал
`mergeable_state=blocked`. PR закрыт без слияния. Это доказательство отказа
prerequisite, а не отдельный искусственный отказ Rust-теста. Реальные результаты
каждой версии нужно смотреть в её [CI run](https://github.com/myjname/ModuleEngine/actions).

## Локальная проверка политики

```sh
python -B -m unittest discover -s scripts -p 'test_*.py' -v
python scripts/ci_plan.py --base <full-base-SHA>
python scripts/verify.py --profile clippy --plan
python scripts/verify.py --profile clippy
python scripts/verify.py --profile native-tools --plan
```

Policy fixtures проверяют выбор путей, безопасный Git argv и отказ aggregate на
ошибке/skip/cancel. Это не hosted CI acceptance. Изменённый workflow нужно проверить
в GitHub, включая fresh dependency restore на обеих системах.

Основания: [required checks и skipped jobs](https://docs.github.com/en/pull-requests/how-tos/merge-and-close-pull-requests/troubleshooting-required-status-checks),
[минимальные permissions и SHA pins](https://docs.github.com/en/actions/reference/security/secure-use),
[образы размещённых раннеров](https://github.com/actions/runner-images).
