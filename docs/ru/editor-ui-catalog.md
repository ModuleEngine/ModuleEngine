# Полный каталог UI редактора

[English](../editor-ui-catalog.md) | [Русский](editor-ui-catalog.md) | [Документация](README.md)

**Дизайн-предложение от 14 ноября 2026; реализация редактора не менялась.** Это ограниченный текущими исходниками реестр 74 элементов: 19 встроенных типов окон, 2 настоящие панели модулей и контракт будущих расширений, 13 вложенных/планируемых видов, 2 элемента каркаса, 7 существующих целей модальных окон приложения и 5 предлагаемых подтверждений/форм, 7 целей системных picker и отдельное подтверждение перезаписи ОС, 11 меню и 4 семейства общих элементов.

«Полный» означает покрытие перечисленных нынешних production-регистраций/целей и уже запрошенных направлений UI. Это не все возможные сочетания данных и не дизайн будущего неизвестного плагина. Новая регистрация требует своей записи/макета. Четыре разных контекста графа — root, GPU, Gameplay Systems, Rust — сохраняют разные данные, исполнение и права.

[Атлас макетов](../media/editor-ui-catalog/index.html) позволяет выбрать каждый интерфейс и переключить основной вид/состояния. Это средство чтения документации, его кнопки не управляют движком.

[Уточнение визуальной иерархии](editor-ui-hierarchy.md) дорабатывает P01 Engine Overview, P08 Inspector и P09 Assets: группировку, выбор, основные действия и три дополнительных SVG узкой области. В атласе они доступны через Compact area. Остальные компоновки этим уточнением не переработаны.

## Правила и границы

[Дизайн](editor-ui-design.md) и [размеры/токены](editor-ui-spec.md) остаются общей базой. Игровой HUD, экраны меню самой игры, будущие G2-графы анимации/ИИ и управление отдельными движками не входят в нынешний UI редактора. В Game они могут быть содержимым проекта; шаблон P22 не навязывает игре бой, инвентарь или головоломку.

Состояния: готово, пусто/нет выбора, занято, черновик, ошибка/отклонение, ограничение, отсутствующая/устаревшая зависимость, узкая область. Каждая запись содержит все восемь с конкретными уточнениями; неприменимость объясняется. Отдельно заданы фокус/Cancel и сохранение. Главное изображение показывает предлагаемую компоновку, лист состояний — локальные варианты/сообщения, а не восемь работающих сборок.

Exact JSON, null, неизвестные данные, stable IDs, author/Play, Full/Team, pins и запреты отсутствующих модулей сохраняются. Ошибка публикации может оставить разрешённые контрактом recovery-файлы: нужно указать их пути, а не обещать отсутствие любых следов записи. Сохранено не равно применено. Поле read-only/Unknown не заменяется выдуманным default.

## Уточнённые предлагаемые решения

Это конкретные варианты для оценки дизайна, не утверждение согласования новой реализации пользователем:

- D08: + копирует текущую раскладку после ввода имени; доступные пресеты — альтернативы. D09/D10 работают с личными раскладками; D12 явно сбрасывает выбранную.
- S02: перенос области, не дублирование; Close возвращает её в главное окно, не закрывает проект.
- P09: Show address открывает выделяемый read-only путь относительно проекта; Copy absolute path отдельно, без ручного перехода по вводу.
- P17–P19 остаются legacy-маршрутом со стабильными ID; смена пресета идёт через D11.
- Иконки — общая собственная векторная геометрия; overlays сначала имеют фиксированные разумные места/переполнение.

## Размеры макетов

Основные SVG: 1280×900 логических единиц листа; листы состояний 1280×1024. Внешняя подпись — аннотация документа, не часть продукта. Общая панель 32 + проектная 36 + статус 24; заголовок области 32. Строки UI/меню 28, toolbar-иконка 20 в зоне 32, Scene 20 в зоне 36. Стандартная app-форма 560, расширенная 720; поля растут/переносятся, body прокручивается, footer остаётся доступен. Общий menu-контейнер до 336, строки не обрезают двухстрочную подпись. Для нативных picker размеры схемы не предписывают ОС реальную геометрию. Адаптация к узкому экрану описана в каждой записи и общей спецификации.

## Реестр и трассировка

| ID | Интерфейс | Статус | Требования | Макет / состояния |
| --- | --- | --- | --- | --- |
| P01 | Обзор Engine | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p01) | [SVG](../media/editor-ui-catalog/p01.svg) / [SVG](../media/editor-ui-catalog/p01-states.svg) / [Узкий](../media/editor-ui-catalog/p01-compact.svg) |
| P02 | Module Blueprint / корневой граф | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p02) | [SVG](../media/editor-ui-catalog/p02.svg) / [SVG](../media/editor-ui-catalog/p02-states.svg) |
| P03 | Библиотека модулей | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p03) | [SVG](../media/editor-ui-catalog/p03.svg) / [SVG](../media/editor-ui-catalog/p03-states.svg) |
| P04 | Разрешения Project в Engine | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p04) | [SVG](../media/editor-ui-catalog/p04.svg) / [SVG](../media/editor-ui-catalog/p04-states.svg) |
| P05 | Настройки Project | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p05) | [SVG](../media/editor-ui-catalog/p05.svg) / [SVG](../media/editor-ui-catalog/p05-states.svg) |
| P06 | Авторская сцена | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p06) | [SVG](../media/editor-ui-catalog/p06.svg) / [SVG](../media/editor-ui-catalog/p06-states.svg) |
| P07 | Иерархия сцены | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p07) | [SVG](../media/editor-ui-catalog/p07.svg) / [SVG](../media/editor-ui-catalog/p07-states.svg) |
| P08 | Инспектор авторского объекта | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p08) | [SVG](../media/editor-ui-catalog/p08.svg) / [SVG](../media/editor-ui-catalog/p08-states.svg) / [Узкий](../media/editor-ui-catalog/p08-compact.svg) |
| P09 | Ресурсы | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p09) | [SVG](../media/editor-ui-catalog/p09.svg) / [SVG](../media/editor-ui-catalog/p09-states.svg) / [Узкий](../media/editor-ui-catalog/p09-compact.svg) |
| P10 | Запуск игры | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p10) | [SVG](../media/editor-ui-catalog/p10.svg) / [SVG](../media/editor-ui-catalog/p10-states.svg) |
| P11 | Игровые системы | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p11) | [SVG](../media/editor-ui-catalog/p11.svg) / [SVG](../media/editor-ui-catalog/p11-states.svg) |
| P12 | Custom / Rust — рядом | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p12) | [SVG](../media/editor-ui-catalog/p12.svg) / [SVG](../media/editor-ui-catalog/p12-states.svg) |
| P13 | Custom / HLSL | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p13) | [SVG](../media/editor-ui-catalog/p13.svg) / [SVG](../media/editor-ui-catalog/p13-states.svg) |
| P14 | Текст проекта | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p14) | [SVG](../media/editor-ui-catalog/p14.svg) / [SVG](../media/editor-ui-catalog/p14-states.svg) |
| P15 | История / слияние | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p15) | [SVG](../media/editor-ui-catalog/p15.svg) / [SVG](../media/editor-ui-catalog/p15-states.svg) |
| P16 | Диагностика | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p16) | [SVG](../media/editor-ui-catalog/p16.svg) / [SVG](../media/editor-ui-catalog/p16-states.svg) |
| P17 | 3D Viewport — старый preview | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p17) | [SVG](../media/editor-ui-catalog/p17.svg) / [SVG](../media/editor-ui-catalog/p17-states.svg) |
| P18 | Object Tree — старый runtime | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p18) | [SVG](../media/editor-ui-catalog/p18.svg) / [SVG](../media/editor-ui-catalog/p18-states.svg) |
| P19 | Object / Components — старые свойства | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p19) | [SVG](../media/editor-ui-catalog/p19.svg) / [SVG](../media/editor-ui-catalog/p19-states.svg) |
| P20 | Participating Modules — панель 100 | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p20) | [SVG](../media/editor-ui-catalog/p20.svg) / [SVG](../media/editor-ui-catalog/p20-states.svg) |
| P21 | Палитра уровней Crate Garden — инструмент 110 | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#p21) | [SVG](../media/editor-ui-catalog/p21.svg) / [SVG](../media/editor-ui-catalog/p21-states.svg) |
| P22 | Контракт панели расширения | Контракт будущего расширения, не регистрация | [→](editor-ui-windows.md#p22) | [SVG](../media/editor-ui-catalog/p22.svg) / [SVG](../media/editor-ui-catalog/p22-states.svg) |
| V01 | GPU-граф рендера | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-views.md#v01) | [SVG](../media/editor-ui-catalog/v01.svg) / [SVG](../media/editor-ui-catalog/v01-states.svg) |
| V02 | Маршрут настройки рендера | Предложенный интерфейс; не реализован | [→](editor-ui-views.md#v02) | [SVG](../media/editor-ui-catalog/v02.svg) / [SVG](../media/editor-ui-catalog/v02-states.svg) |
| V03 | Assets / корзина | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-views.md#v03) | [SVG](../media/editor-ui-catalog/v03.svg) / [SVG](../media/editor-ui-catalog/v03-states.svg) |
| V04 | Разрешение конфликтов слияния | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-views.md#v04) | [SVG](../media/editor-ui-catalog/v04.svg) / [SVG](../media/editor-ui-catalog/v04-states.svg) |
| V05 | Инспектор ноды модуля | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-views.md#v05) | [SVG](../media/editor-ui-catalog/v05.svg) / [SVG](../media/editor-ui-catalog/v05-states.svg) |
| V06 | Инспектор визуальной ноды Rust | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-views.md#v06) | [SVG](../media/editor-ui-catalog/v06.svg) / [SVG](../media/editor-ui-catalog/v06-states.svg) |
| V07 | Rust / текстовый режим | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-views.md#v07) | [SVG](../media/editor-ui-catalog/v07.svg) / [SVG](../media/editor-ui-catalog/v07-states.svg) |
| V08 | Rust / графовый режим | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-views.md#v08) | [SVG](../media/editor-ui-catalog/v08.svg) / [SVG](../media/editor-ui-catalog/v08-states.svg) |
| V09 | Восстановление состояния Custom | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-views.md#v09) | [SVG](../media/editor-ui-catalog/v09.svg) / [SVG](../media/editor-ui-catalog/v09-states.svg) |
| V10 | Карта архитектуры | Предложенный интерфейс; не реализован | [→](editor-ui-views.md#v10) | [SVG](../media/editor-ui-catalog/v10.svg) / [SVG](../media/editor-ui-catalog/v10-states.svg) |
| S01 | Каркас редактора и раскладки | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-windows.md#s01) | [SVG](../media/editor-ui-catalog/s01.svg) / [SVG](../media/editor-ui-catalog/s01-states.svg) |
| S02 | Дополнительное нативное окно | Предложенный интерфейс; не реализован | [→](editor-ui-windows.md#s02) | [SVG](../media/editor-ui-catalog/s02.svg) / [SVG](../media/editor-ui-catalog/s02-states.svg) |
| N01 | Открытие проекта | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#n01) | [SVG](../media/editor-ui-catalog/n01.svg) / [SVG](../media/editor-ui-catalog/n01-states.svg) |
| N02 | Сохранение проекта как | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#n02) | [SVG](../media/editor-ui-catalog/n02.svg) / [SVG](../media/editor-ui-catalog/n02-states.svg) |
| N03 | Папка нового проекта | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#n03) | [SVG](../media/editor-ui-catalog/n03.svg) / [SVG](../media/editor-ui-catalog/n03-states.svg) |
| N04 | Папка Windows Player | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#n04) | [SVG](../media/editor-ui-catalog/n04.svg) / [SVG](../media/editor-ui-catalog/n04-states.svg) |
| N05 | Папка редактора Team | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#n05) | [SVG](../media/editor-ui-catalog/n05.svg) / [SVG](../media/editor-ui-catalog/n05-states.svg) |
| N06 | Папка полных исходников | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#n06) | [SVG](../media/editor-ui-catalog/n06.svg) / [SVG](../media/editor-ui-catalog/n06-states.svg) |
| N07 | Выбор импортируемого ресурса | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#n07) | [SVG](../media/editor-ui-catalog/n07.svg) / [SVG](../media/editor-ui-catalog/n07-states.svg) |
| N08 | Системное подтверждение перезаписи | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#n08) | [SVG](../media/editor-ui-catalog/n08.svg) / [SVG](../media/editor-ui-catalog/n08-states.svg) |
| D01 | Несохранённые изменения / черновики | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#d01) | [SVG](../media/editor-ui-catalog/d01.svg) / [SVG](../media/editor-ui-catalog/d01-states.svg) |
| D02 | Результат / отчёт операции | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#d02) | [SVG](../media/editor-ui-catalog/d02.svg) / [SVG](../media/editor-ui-catalog/d02-states.svg) |
| D03 | Новая папка | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#d03) | [SVG](../media/editor-ui-catalog/d03.svg) / [SVG](../media/editor-ui-catalog/d03-states.svg) |
| D04 | Переименование ресурса | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#d04) | [SVG](../media/editor-ui-catalog/d04.svg) / [SVG](../media/editor-ui-catalog/d04-states.svg) |
| D05 | Перенос ресурса | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#d05) | [SVG](../media/editor-ui-catalog/d05.svg) / [SVG](../media/editor-ui-catalog/d05-states.svg) |
| D06 | Удаление ресурса / разрешение использований | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#d06) | [SVG](../media/editor-ui-catalog/d06.svg) / [SVG](../media/editor-ui-catalog/d06-states.svg) |
| D07 | Использования ресурса | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#d07) | [SVG](../media/editor-ui-catalog/d07.svg) / [SVG](../media/editor-ui-catalog/d07-states.svg) |
| D08 | Создание раскладки | Предложенный интерфейс; не реализован | [→](editor-ui-dialogs.md#d08) | [SVG](../media/editor-ui-catalog/d08.svg) / [SVG](../media/editor-ui-catalog/d08-states.svg) |
| D09 | Переименование раскладки | Предложенный интерфейс; не реализован | [→](editor-ui-dialogs.md#d09) | [SVG](../media/editor-ui-catalog/d09.svg) / [SVG](../media/editor-ui-catalog/d09-states.svg) |
| D10 | Удаление личной раскладки | Предложенный интерфейс; не реализован | [→](editor-ui-dialogs.md#d10) | [SVG](../media/editor-ui-catalog/d10.svg) / [SVG](../media/editor-ui-catalog/d10-states.svg) |
| D11 | Проверка замены пресета рендера | Предложенный интерфейс; не реализован | [→](editor-ui-dialogs.md#d11) | [SVG](../media/editor-ui-catalog/d11.svg) / [SVG](../media/editor-ui-catalog/d11-states.svg) |
| D12 | Сброс выбранной раскладки | Предложенный интерфейс; не реализован | [→](editor-ui-dialogs.md#d12) | [SVG](../media/editor-ui-catalog/d12.svg) / [SVG](../media/editor-ui-catalog/d12-states.svg) |
| M01 | File и подменю | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-menus.md#m01) | [SVG](../media/editor-ui-catalog/m01.svg) / [SVG](../media/editor-ui-catalog/m01-states.svg) |
| M02 | Меню Edit | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-menus.md#m02) | [SVG](../media/editor-ui-catalog/m02.svg) / [SVG](../media/editor-ui-catalog/m02-states.svg) |
| M03 | Меню Tools | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-menus.md#m03) | [SVG](../media/editor-ui-catalog/m03.svg) / [SVG](../media/editor-ui-catalog/m03-states.svg) |
| M04 | Меню Windows | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-menus.md#m04) | [SVG](../media/editor-ui-catalog/m04.svg) / [SVG](../media/editor-ui-catalog/m04-states.svg) |
| M05 | Меню рабочей раскладки | Предложенный интерфейс; не реализован | [→](editor-ui-menus.md#m05) | [SVG](../media/editor-ui-catalog/m05.svg) / [SVG](../media/editor-ui-catalog/m05-states.svg) |
| M06 | Меню границы областей | Предложенный интерфейс; не реализован | [→](editor-ui-menus.md#m06) | [SVG](../media/editor-ui-catalog/m06.svg) / [SVG](../media/editor-ui-catalog/m06-states.svg) |
| M07 | Поиск добавляемой ноды | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-menus.md#m07) | [SVG](../media/editor-ui-catalog/m07.svg) / [SVG](../media/editor-ui-catalog/m07-states.svg) |
| M08 | Действия ноды | Предложенный интерфейс; не реализован | [→](editor-ui-menus.md#m08) | [SVG](../media/editor-ui-catalog/m08.svg) / [SVG](../media/editor-ui-catalog/m08-states.svg) |
| M09 | Контекстное меню ресурса | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-menus.md#m09) | [SVG](../media/editor-ui-catalog/m09.svg) / [SVG](../media/editor-ui-catalog/m09-states.svg) |
| M10 | Выбор типа редактора области | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-menus.md#m10) | [SVG](../media/editor-ui-catalog/m10.svg) / [SVG](../media/editor-ui-catalog/m10-states.svg) |
| M11 | Настройки инструментов Scene | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-menus.md#m11) | [SVG](../media/editor-ui-catalog/m11.svg) / [SVG](../media/editor-ui-catalog/m11-states.svg) |
| C01 | Поля по схемам | Общее семейство элементов; предлагаемый стандарт | [→](editor-ui-views.md#c01) | [SVG](../media/editor-ui-catalog/c01.svg) / [SVG](../media/editor-ui-catalog/c01-states.svg) |
| C02 | Выбор ссылок на объекты/ресурсы | Общее семейство элементов; предлагаемый стандарт | [→](editor-ui-views.md#c02) | [SVG](../media/editor-ui-catalog/c02.svg) / [SVG](../media/editor-ui-catalog/c02-states.svg) |
| C03 | Справка и обратная связь иконок/пинов | Общее семейство элементов; предлагаемый стандарт | [→](editor-ui-views.md#c03) | [SVG](../media/editor-ui-catalog/c03.svg) / [SVG](../media/editor-ui-catalog/c03-states.svg) |
| C04 | Трансформации, цвет и стандартные элементы | Общее семейство элементов; предлагаемый стандарт | [→](editor-ui-views.md#c04) | [SVG](../media/editor-ui-catalog/c04.svg) / [SVG](../media/editor-ui-catalog/c04-states.svg) |
| V11 | GPU-граф / пример Deferred | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-views.md#v11) | [SVG](../media/editor-ui-catalog/v11.svg) / [SVG](../media/editor-ui-catalog/v11-states.svg) |
| V12 | Rust / пример scalar-графа | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-views.md#v12) | [SVG](../media/editor-ui-catalog/v12.svg) / [SVG](../media/editor-ui-catalog/v12-states.svg) |
| V13 | JSON игрового расписания | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-views.md#v13) | [SVG](../media/editor-ui-catalog/v13.svg) / [SVG](../media/editor-ui-catalog/v13-states.svg) |
| D13 | Операция с исходником сцены | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#d13) | [SVG](../media/editor-ui-catalog/d13.svg) / [SVG](../media/editor-ui-catalog/d13-states.svg) |
| D14 | Переименование объекта | Существующий интерфейс; предлагаемая вёрстка | [→](editor-ui-dialogs.md#d14) | [SVG](../media/editor-ui-catalog/d14.svg) / [SVG](../media/editor-ui-catalog/d14-states.svg) |

## Проверка и воспроизведение

Канонические двуязычные данные — [catalog.json](../ui-catalog/catalog.json), генератор — [generate.py](../ui-catalog/generate.py). Генерация не меняет редактор, проекты или личные настройки. Требуется только стандартная библиотека Python 3.9+.

```text
python docs/ui-catalog/generate.py
python docs/ui-catalog/generate.py --check
```

Проверка охватывает сопоставление source-регистраций/диалогов с реестром, EN/RU/состояния, наличие/согласованность всех макетов, ссылки, геометрию и контраст. QA требует отрисовки всех SVG и осмотра каждого листа; тест атласа проверяет выбор/поиск/состояния, а не команды движка. Итоговое независимое заключение и привязка источников — в `outputs/reviews/editor-ui-catalog-20261114/`. Скриншоты не доказывают нативный ввод, DPI, доступность ОС, сохранение, FPS или выполнение требований в продукте.
