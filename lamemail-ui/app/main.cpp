#include "instance.h"
#include "lamemail.h"
#include "startup.h"

#include <QCommandLineParser>
#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickWindow>

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
    parser.addOption(dir);
    parser.process(app);
    lamemail::setDir(lamemail::syncDir(parser.value(dir)));

    Instance instance(lamemail::dir());
    if (instance.handOver(token))
        return 0;

    QQmlApplicationEngine engine;
    QQuickWindow* window = lamemail::open(engine);
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
