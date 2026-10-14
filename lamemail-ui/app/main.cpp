#include "instance.h"
#include "lamemail.h"
#include "preferences.h"
#include "startup.h"

#include <QCommandLineParser>
#include <QFileInfo>
#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QStandardPaths>

int main(int argc, char* argv[])
{
    const QByteArray token = qgetenv("XDG_ACTIVATION_TOKEN");
    lamemail::identify();
    QGuiApplication app(argc, argv);
    QGuiApplication::setWindowIcon(QIcon(QStringLiteral(":/lamemail.svg")));

    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral("Shows a lamemail sync folder as one feed"));
    parser.addHelpOption();
    parser.addVersionOption();
    const QCommandLineOption dir = lamemail::dirOption();
    const QCommandLineOption hidden(QStringLiteral("hidden"),
        QStringLiteral("Start in the tray, without the window, when closing to the tray is on"));
    parser.addOptions({ dir, hidden });
    parser.process(app);
    lamemail::setDir(lamemail::syncDir(parser.value(dir)));

    Instance instance(lamemail::dir());
    if (instance.handOver(token))
        return 0;

    const QString image = qEnvironmentVariable("APPIMAGE");
    QStringList launch { image.isEmpty() ? QCoreApplication::applicationFilePath() : image };
    if (parser.isSet(dir))
        launch << QStringLiteral("--dir") << QFileInfo(lamemail::dir()).absoluteFilePath();
    launch << QStringLiteral("--hidden");
    Preferences::setLaunch(launch,
        QStandardPaths::writableLocation(QStandardPaths::GenericConfigLocation)
            + QStringLiteral("/autostart/lamemail-ui.desktop"));

    QQmlApplicationEngine engine;
    QQuickWindow* window = lamemail::open(engine, parser.isSet(hidden));
    if (!window)
        return 1;
    instance.listen();
    QObject::connect(&instance, &Instance::activated, window, [window](const QByteArray& handed) {
        if (!handed.isEmpty())
            qputenv("XDG_ACTIVATION_TOKEN", handed);
        QMetaObject::invokeMethod(window, "present");
    });
    return QGuiApplication::exec();
}
