#include "instance.h"
#include "lamemail.h"
#include "mailto.h"
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
    const QCommandLineOption message(QStringLiteral("open"),
        QStringLiteral("Open a message, its path as `lamemail list --json` prints it"),
        QStringLiteral("PATH"));
    parser.addOptions({ dir, hidden, message });
    parser.addPositionalArgument(QStringLiteral("link"),
        QStringLiteral("A mailto: link to write a new mail to, as the desktop hands it"),
        QStringLiteral("[mailto:…]"));
    parser.process(app);
    lamemail::setDir(lamemail::syncDir(parser.value(dir)));
    const QStringList links = parser.positionalArguments();
    const QString target = parser.isSet(message) ? parser.value(message)
        : links.isEmpty()                        ? QString()
                                                 : Mailto::link(links.constFirst());

    Instance instance(lamemail::dir());
    if (instance.handOver(token, target))
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
    QQuickWindow* window = lamemail::open(engine, parser.isSet(hidden) && target.isEmpty());
    if (!window)
        return 1;
    instance.listen();
    QObject::connect(&instance, &Instance::activated, window,
        [window](const QByteArray& handed, const QString& path) {
            if (!handed.isEmpty())
                qputenv("XDG_ACTIVATION_TOKEN", handed);
            lamemail::reveal(window, path);
        });
    if (!target.isEmpty())
        lamemail::reveal(window, target);
    return QGuiApplication::exec();
}
