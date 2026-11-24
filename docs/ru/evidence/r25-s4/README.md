# Изображения S4 и эталон временной памяти для R2.5

[English](../../../evidence/r25-s4/README.md) | [Русский](README.md) | [Документация](../../README.md)

S4 на `main` в коммите `9aa0e7633b2bfbf87fbd7d89bb6c6085cc04c8c5`
использует **203 489 280 байт (194,0625 МиБ)** для изображений графа Forward
и **238 878 720 байт (227,8125 МиБ)** для Deferred при 1920×1080. Эталон
учитывает один завершённый внеэкранный слот штатного графа со всеми
включёнными проходами.

| Граф | Изображения | Память изображений графа, байт | Опубликованный эталонный кадр |
| --- | ---: | ---: | --- |
| Forward, SSAO, ACES tonemap, FXAA, цветовое кодирование | 8 | 203 489 280 | [Камера 0, PNG](../../../evidence/r25-s4/forward-000.png) |
| G-buffer, Deferred lighting, SSAO, ACES tonemap, FXAA, цветовое кодирование | 9 | 238 878 720 | [Камера 31, PNG](../../../evidence/r25-s4/deferred-031.png) |

Два эталонных кадра в полном разрешении доступны вместе с исходниками.
`baseline.json` содержит для каждого графа хеши PNG и отчёта захвата,
положение камеры, разрешение и хеш отчёта о диапазонах памяти. Полные
серии по 32 кадра и исходные манифесты сохранены в локальном каталоге
поставки `outputs/rv-c/`. [Манифест](../../../evidence/r25-s4/baseline.json)

## Исходники и условия рендера

Стенд использовал Windows x64 `26300.10075`, Ryzen 9 3900X, RTX 4080 SUPER
`10de:2702`, драйвер NVIDIA `617.42`, Vulkan устройства `1.4.351`, Rust
`1.99.0` и DXC `1.9.2609.5`. Editor собран в Release с `bench`.
Опубликованный кадр Forward снят с неизменённого `main`. Диагностическая
сборка добавляет режим захвата Deferred и читает существующие привязки
памяти Vulkan. Все 32 PNG Forward и манифест захвата из этой сборки
побайтно совпали с захватом с неизменённого `main`.

Штатный glTF-импортёр загружает закреплённую версию Sponza и две модели
Khronos. Оба графа используют индексируемые изображения базового цвета
и развёрнутый поток геометрии из снимка сцены. Для камеры `i` (0–31) цель
равна `[-5 - 0.025*i, 0.45, 0.0125*i]`, расстояние `13`, наклон `0.10`,
азимут `pi/2`. В сцене описаны 256 точечных источников. Действующие
шейдеры используют встроенное направленное освещение. Захват читает RGBA
через диагностику Vulkan при 1920×1080.
[Фикстура](../../../../crates/editor/src/bench/fixtures.rs) · [Камера и захват](../../../../crates/editor/src/bench/mod.rs)

Изображения Sponza показывают glTF-конверсию Khronos модели, созданной
Марко Дабровичем и доработанной Crytek. Входной манифест закрепляет 71 файл.
[Входной манифест](../../../../examples/bench/sponza/manifest.json) · [Авторы и условия](https://github.com/KhronosGroup/glTF-Sample-Assets/blob/e124e8d8d7330a5372bc2b4d00d39a4a9d7997ea/Models/Sponza/README.md)

## Повтор сравнения

В клоне с закреплённым нативным toolchain и адаптером Vulkan восстановите
DXC обычным bootstrap и запустите команды из корня. Для каждого захвата
задайте новый выходной каталог. Запускайте оба захвата последовательно
на эталонном адаптере.

```powershell
python -B -c "import sys; sys.path.insert(0, 'scripts'); from bench import ensure_bench_assets; print(ensure_bench_assets())"
cargo build --locked --release -p module-engine-editor --features bench
& target/release/module-engine-editor.exe --bench-scene S4-floor rv-c-s4-forward
& target/release/module-engine-editor.exe --bench-scene S4-floor-deferred rv-c-s4-deferred
Get-FileHash docs/evidence/r25-s4/forward-000.png, rv-c-s4-forward/frame-000.png
Get-FileHash docs/evidence/r25-s4/deferred-031.png, rv-c-s4-deferred/frame-031.png
python -B scripts/s4_graph_memory.py rv-c-s4-forward/graph-memory.json --reference docs/evidence/r25-s4/forward-memory.json
python -B scripts/s4_graph_memory.py rv-c-s4-deferred/graph-memory.json --reference docs/evidence/r25-s4/deferred-memory.json
```

На Linux у исполняемого файла нет суффикса `.exe`. Хеши в манифесте
идентифицируют байты сохранённых PNG. Совпадение хешей при повторе проверено
на эталонном устройстве Windows. Для другой GPU или изменённого графа
сравнивайте декодированные пиксели по согласованному допуску R2.5.
[Toolchain](../../../../toolchain-manifest.json) · [Руководство по бенчмаркам](../../bench.md)

Для сравнения памяти берётся объединение диапазонов `[offset, offset +
VkMemoryRequirements::size)` в байтах по каждой отдельной привязке
`VkDeviceMemory`. Учитываются все изображения графа в завершённом слоте,
включая аппаратную глубину и временный закодированный выход. Пересекающиеся
диапазоны с aliasing считаются один раз. Свободное место в аллокаторе,
постоянные изображения материалов, геометрия, буферы чтения и внешние
изображения презентации в метрику не входят. В двух сохранённых графах
диапазоны не пересекаются. Для критерия памяти R2.5 при повторном замере
используются тот же граф, разрешение, путь материалов и определение слота.
[Диапазоны Forward](../../../evidence/r25-s4/forward-memory.json) · [Диапазоны Deferred](../../../evidence/r25-s4/deferred-memory.json) · [Скрипт сравнения](../../../../scripts/s4_graph_memory.py)
