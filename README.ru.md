# ModuleEngine

[English](README.md) | [Русский](README.ru.md) | [Документация](docs/ru/README.md)

ModuleEngine — модульный движок на Rust с нативным редактором и отдельным Player. Engine определяет доступные модули и правила редактирования. Project настраивает игру в этих границах. Core владеет ECS и общими контрактами. Игровые пакеты определяют конкретные правила, а игровой рендерер работает на Vulkan.

[Версия сборки в Cargo](Cargo.toml) — `0.4.0`. Владелец закрыл этап 0.4.2 и назначил [перестройку движка R](docs/ru/product-roadmap.md#rebuild) текущим приоритетом. R1.8 завершил переход хранилища на ECS с Bevy, а R2.4 добавил штатное отсечение на GPU. Реализованный объём и границы проверок описаны в [текущем состоянии](docs/ru/current-status.md), предыдущие отчёты находятся в [архиве](docs/ru/status-archive.md).

![Scene, Hierarchy, Inspector и Assets в редакторе](docs/media/editor-scene-assets.png)

<a id="running"></a>

## Запуск

Для запуска Editor на Windows из checkout используйте закреплённые инструменты и Cargo. Doctor сообщает, каких инструментов не хватает на компьютере. Порядок установки локальных инструментов и зависимостей указан в [руководстве по toolchain](docs/ru/toolchain.md). Для Linux см. [требования и поддержку оконных систем](docs/ru/linux-support.md).

~~~powershell
.\scripts\bootstrap.ps1 -Json
cargo run --locked -p module-engine-editor
~~~

В готовом пакете Windows запустите `.\ModuleEngine.exe` из каталога пакета. Чтобы открыть конкретный проект, передайте путь к JSON. Для переноса старого формата в отдельный файл используйте `--migrate-copy <source.json> <new-project.json> <neutral|arpg|puzzle|g1>`. См. [профили Engine](docs/ru/engine-profiles.md).

## Engine и Project

| Дистрибутив | Назначение |
| --- | --- |
| Full | Меняйте архитектуру и исходники Engine при полном совместимом манифесте исходников. Редактируйте данные Project. |
| Team | Редактируйте открытые настройки Project, сцены и разрешённый игровой Rust в рамках замороженного Engine. Для незнакомого проекта Team компиляция ждёт [разрешения для проекта](docs/ru/distribution.md#team-custom-rust-permission). |
| Player | Запускайте упакованную игру с готовой логикой и шейдерами. |

<a id="editor-areas"></a>

## Области редактора

Начните с **File → New Project → Empty scene** или выберите пример. **Scene**, **Hierarchy**, **Inspector** и **Assets** работают с одним авторским проектом и общим выбором объектов. **Game** запускает отдельную Play-копию. Crate Garden — головоломка из десяти уровней. Two Arenas показывает игровой процесс ARPG и путь до Player. См. [рабочий процесс](docs/ru/editor-workflow.md), [Scene и Assets](docs/ru/scene-assets.md), [Crate Garden](docs/ru/g1-game.md) и [создание игр](docs/ru/editor-game.md).

<a id="cli-and-export"></a>

## CLI и экспорт

~~~powershell
.\ModuleEngine.exe --check project.json
.\ModuleEngine.exe --render project.json scene.png
.\ModuleEngine.exe --export-game project.json player-build
~~~

Выполняйте эти команды в каталоге пакета Editor с JSON проекта или подставьте пути к своему EXE и проекту. Для экспорта Player выберите новый или пустой каталог. Готовый Player открывает `player.json` и использует собранные Rust-воркеры и SPIR-V. Для экспорта редакторов Full и Team нужны совместимые исходники Full. См. [дистрибутивы](docs/ru/distribution.md) и [экспорт Player](docs/ru/player.md).

<a id="verification"></a>

## Проверка

Начните с плана [профиля проверки](docs/ru/verification.md#профили). В обычном Editor доступны команды `--check` и `--render` для проекта. Для проверок редактора, например `--self-check`, собирайте с `--features verification`. Для [нативных бенчмарков сцены](docs/ru/bench.md) используйте `bench`.

<a id="documentation"></a>

## Документация

- [Архитектура и контракты Core](docs/ru/architecture.md) · [ECS](docs/ru/ecs-world.md) · [GameHost](docs/ru/shared-game-host.md)
- [Исполняемые модули](docs/ru/module-execution.md) · [Схемы компонентов](docs/ru/component-schemas.md) · [Custom Rust](docs/ru/custom-rust.md) · [Состояние Custom](docs/ru/custom-state.md)
- [Дорожная карта](docs/ru/product-roadmap.md) · [Работа над ModuleEngine](docs/ru/agent-guide.md) · [Оглавление документации](docs/ru/README.md)

<a id="лицензия"></a>

## Лицензия

ModuleEngine распространяется на выбор по [лицензии MIT](LICENSE-MIT) или [Apache 2.0](LICENSE-APACHE). Экспорт включает обе лицензии и [уведомления сторонних пакетов](THIRD-PARTY-LICENSES.txt). Лицензии ресурсов и включённых пакетов указаны в разделе [лицензирование и атрибуция](docs/ru/licensing.md).
