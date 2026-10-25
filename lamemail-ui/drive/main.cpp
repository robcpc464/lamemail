#include "lamemail.h"
#include "preferences.h"
#include "startup.h"

#include <QCommandLineParser>
#include <QDesktopServices>
#include <QDir>
#include <QGuiApplication>
#include <QHash>
#include <QImage>
#include <QKeySequence>
#include <QPointingDevice>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QRegularExpression>
#include <QSettings>
#include <QTest>
#include <QTextStream>
#include <QUrl>
#include <QWheelEvent>
#include <optional>

namespace {

constexpr int loadMs = 1500;
constexpr int settleMs = 400;
constexpr int dragSteps = 20;
constexpr int notch = 120;
constexpr int notchPixels = 15;

void say(FILE* to, const QString& line) { QTextStream(to) << line << Qt::endl; }

std::optional<QList<int>> numbers(const QStringList& words, qsizetype count)
{
    if (words.size() != count)
        return std::nullopt;
    QList<int> values;
    for (const QString& word : words) {
        bool ok = false;
        values.append(word.toInt(&ok));
        if (!ok)
            return std::nullopt;
    }
    return values;
}

class Drive {
public:
    Drive(QQuickWindow* window, const QDir& out)
        : m_window(window)
        , m_touchpad(
              new QPointingDevice(QStringLiteral("touchpad"), 0, QInputDevice::DeviceType::TouchPad,
                  QPointingDevice::PointerType::Finger, QInputDevice::Capability::Position, 1, 0,
                  QString(), QPointingDeviceUniqueId(), window))
        , m_out(out)
    {
    }

    bool run(const QString& step)
    {
        const QString command = step.section(QLatin1Char(' '), 0, 0, QString::SectionSkipEmpty);
        const QString rest = step.section(QLatin1Char(' '), 1, -1, QString::SectionSkipEmpty);
        const bool done = command == QLatin1String("type") ? type(rest)
            : command == QLatin1String("key")              ? press(rest)
            : command == QLatin1String("shot")             ? capture(rest)
            : command == QLatin1String("open")             ? reveal(rest)
                                                           : point(command, rest);
        if (!done)
            say(stderr, QStringLiteral("not a step: `%1`").arg(step));
        return done;
    }

private:
    bool type(const QString& text)
    {
        for (const QChar letter : text)
            QTest::sendKeyEvent(QTest::Click, m_window, Qt::Key(letter.toUpper().unicode()),
                QString(letter), Qt::NoModifier);
        return settled(!text.isEmpty());
    }

    bool press(const QString& sequence)
    {
        const QKeySequence keys = QKeySequence::fromString(sequence);
        if (keys.isEmpty() || keys[0].key() == Qt::Key_unknown)
            return false;
        QTest::keySequence(m_window, keys);
        return settled(true);
    }

    bool reveal(const QString& path)
    {
        lamemail::reveal(m_window, path);
        return settled(!path.isEmpty());
    }

    bool capture(const QString& name)
    {
        static const QRegularExpression plain(QStringLiteral("^[a-z0-9-]+$"));
        const QString path = m_out.filePath(name + QLatin1String(".png"));
        if (!plain.match(name).hasMatch() || !m_out.mkpath(QStringLiteral("."))
            || !m_window->grabWindow().save(path))
            return false;
        say(stdout, path);
        return true;
    }

    bool point(const QString& command, const QString& rest)
    {
        static const QHash<QString, qsizetype> counts {
            { QStringLiteral("wait"), 1 },
            { QStringLiteral("size"), 2 },
            { QStringLiteral("move"), 2 },
            { QStringLiteral("click"), 2 },
            { QStringLiteral("wheel"), 3 },
            { QStringLiteral("drag"), 4 },
        };
        QStringList words = rest.split(QLatin1Char(' '), Qt::SkipEmptyParts);
        const QString option
            = command == QLatin1String("click") && words.size() == 3 ? words.takeLast() : QString();
        const bool ctrl = option == QLatin1String("ctrl");
        const bool right = option == QLatin1String("right");
        if (!option.isEmpty() && !ctrl && !right)
            return false;
        const auto values = numbers(words, counts.value(command));
        if (!values || values->isEmpty())
            return false;
        const QList<int>& n = *values;
        if (command == QLatin1String("wait")) {
            QTest::qWait(n[0]);
            return true;
        }
        const QPoint at(n[0], n[1]);
        if (command == QLatin1String("size")) {
            m_window->resize(n[0], n[1]);
            return settled(true);
        }
        QTest::mouseMove(m_window, at);
        if (command == QLatin1String("click")) {
            QTest::mouseClick(m_window, right ? Qt::RightButton : Qt::LeftButton,
                ctrl ? Qt::ControlModifier : Qt::NoModifier, at);
        } else if (command == QLatin1String("wheel")) {
            QWheelEvent turn(at, m_window->mapToGlobal(at), { 0, -notchPixels * n[2] },
                { 0, -notch * n[2] }, Qt::NoButton, Qt::NoModifier, Qt::NoScrollPhase, false,
                Qt::MouseEventNotSynthesized, m_touchpad);
            QCoreApplication::sendEvent(m_window, &turn);
        } else if (command == QLatin1String("drag")) {
            const QPoint to(n[2], n[3]);
            QTest::mousePress(m_window, Qt::LeftButton, { }, at);
            for (int step = 1; step <= dragSteps; ++step)
                QTest::mouseMove(m_window, at + (to - at) * step / dragSteps, 10);
            QTest::mouseRelease(m_window, Qt::LeftButton, { }, to);
        }
        return settled(true);
    }

    static bool settled(bool done)
    {
        QTest::qWait(settleMs);
        return done;
    }

    QQuickWindow* m_window;
    const QPointingDevice* m_touchpad;
    QDir m_out;
};

class Launcher : public QObject {
    Q_OBJECT

public:
    Q_INVOKABLE void open(const QUrl& url)
    {
        say(stdout, QStringLiteral("open ") + url.toString());
    }
};

}

int main(int argc, char* argv[])
{
    qputenv("QT_QPA_PLATFORM", "offscreen");
    qputenv("QT_QUICK_BACKEND", "software");
    qputenv("QT_FORCE_STDERR_LOGGING", "1");
    qputenv("QT_NO_XDG_DESKTOP_PORTAL", "1");
    qunsetenv("QT_QPA_PLATFORMTHEME");
    qunsetenv("QT_IM_MODULE");
    lamemail::identify();
    QGuiApplication app(argc, argv);

    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral(
        "Drives lamemail-ui without a display and captures it, step by step:\n"
        "  size <w> <h>, click <x> <y> [ctrl|right], move <x> <y>, drag <x> <y> <x2> <y2>,\n"
        "  wheel <x> <y> <notches>, key <sequence>, type <text>, open <path>, wait <ms>,\n"
        "  shot <name>"));
    parser.addHelpOption();
    const QCommandLineOption dir = lamemail::dirOption();
    const QCommandLineOption out(QStringLiteral("out"),
        QStringLiteral("Folder for the captures and the settings of the run"),
        QStringLiteral("DIR"), QStringLiteral("."));
    parser.addOptions({ dir, out });
    parser.addPositionalArgument(QStringLiteral("steps"),
        QStringLiteral("One step per argument, e.g. \"key Ctrl+N\" \"shot compose\""),
        QStringLiteral("<step>..."));
    parser.process(app);
    lamemail::setDir(lamemail::syncDir(parser.value(dir)));
    const QDir folder(parser.value(out));
    QSettings::setPath(QSettings::NativeFormat, QSettings::UserScope,
        folder.absoluteFilePath(QStringLiteral("settings")));

    qputenv("XDG_RUNTIME_DIR", folder.absoluteFilePath(QStringLiteral("runtime")).toLocal8Bit());
    Preferences::setLaunch({ QCoreApplication::applicationFilePath(), QStringLiteral("--hidden") },
        folder.absoluteFilePath(QStringLiteral("autostart/lamemail-ui.desktop")));
    Launcher launcher;
    for (const char* scheme : { "http", "https", "file" })
        QDesktopServices::setUrlHandler(QLatin1String(scheme), &launcher, "open");

    QQmlApplicationEngine engine;
    bool clean = true;
    QObject::connect(&engine, &QQmlEngine::warnings, &app, [&clean] { clean = false; });
    QQuickWindow* window = lamemail::open(engine, false);
    if (!window || !QTest::qWaitForWindowExposed(window))
        return 1;
    window->requestActivate();
    QTest::qWait(loadMs);

    Drive drive(window, folder);
    for (const QString& step : parser.positionalArguments()) {
        if (!drive.run(step))
            return 1;
    }
    return clean ? 0 : 1;
}

#include "main.moc"
