#include "lamemail.h"

#include "texts.h"

#include <QCoreApplication>
#include <QDebug>
#include <QDir>
#include <QFileInfo>

namespace {

QString& current()
{
    static QString value;
    return value;
}

}

namespace lamemail {

QString syncDir(const QString& option)
{
    if (!option.isEmpty())
        return option;
    const QString env = qEnvironmentVariable("LAMEMAIL_DIR");
    if (!env.isEmpty())
        return env;
    return QDir::homePath() + QStringLiteral("/.lamemail");
}

void setDir(const QString& dir) { current() = dir; }

QString dir() { return current().isEmpty() ? QStringLiteral(".") : current(); }

QString binary()
{
    const QFileInfo next(QCoreApplication::applicationDirPath() + QStringLiteral("/lamemail"));
    if (next.isFile() && next.isExecutable())
        return next.filePath();
    return QStringLiteral("lamemail");
}

QStringList arguments(const QStringList& command)
{
    return QStringList { QStringLiteral("--dir"), dir() } + command;
}

std::optional<QString> lastLine(const QString& stderrText)
{
    const QStringList lines = stderrText.split(QLatin1Char('\n'));
    for (auto line = lines.crbegin(); line != lines.crend(); ++line) {
        QString trimmed = line->trimmed();
        if (trimmed.isEmpty())
            continue;
        if (trimmed.startsWith(QLatin1String("lamemail: ")))
            trimmed.remove(0, 10);
        return trimmed;
    }
    return std::nullopt;
}

Outcome outcome(QProcess* process, int code, QProcess::ExitStatus status)
{
    const QString errors = QString::fromUtf8(process->readAllStandardError()).trimmed();
    if (status == QProcess::NormalExit && code == 0) {
        if (!errors.isEmpty())
            qWarning().noquote() << errors;
        return { true, process->readAllStandardOutput(), { }, code };
    }
    const QString fallback = status == QProcess::CrashExit
        ? QString::fromUtf8(texts().crashed)
        : QString::fromUtf8(texts().exitedWithCode)
              .replace(QLatin1String("{}"), QString::number(code));
    return { false, process->readAllStandardOutput(), lastLine(errors).value_or(fallback),
        status == QProcess::NormalExit ? code : -1 };
}

void run(QObject* context, const QStringList& command, std::function<void(const Outcome&)> done,
    std::function<void(const QByteArray&)> output, const QByteArray& input)
{
    auto* process = new QProcess(context);
    auto finish = [process, done](const Outcome& outcome) {
        process->deleteLater();
        done(outcome);
    };
    if (output)
        QObject::connect(process, &QProcess::readyReadStandardOutput, context,
            [process, output] { output(process->readAllStandardOutput()); });
    QObject::connect(process, &QProcess::finished, context,
        [process, finish](
            int code, QProcess::ExitStatus status) { finish(outcome(process, code, status)); });
    QObject::connect(process, &QProcess::errorOccurred, context,
        [process, finish](QProcess::ProcessError error) {
            if (error == QProcess::FailedToStart)
                finish({ false, { }, binary() + QStringLiteral(": ") + process->errorString() });
        });
    process->start(binary(), arguments(command));
    if (!input.isEmpty())
        process->write(input);
    process->closeWriteChannel();
}

}
