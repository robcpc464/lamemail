#pragma once

#include <QByteArray>
#include <QObject>
#include <QProcess>
#include <QString>
#include <QStringList>
#include <functional>
#include <optional>

namespace lamemail {

struct Outcome {
    bool ok = false;
    QByteArray out;
    QString error;
    int code = -1;
};

QString syncDir(const QString& option);
void setDir(const QString& dir);
QString dir();
QString binary();
QStringList arguments(const QStringList& command);
std::optional<QString> lastLine(const QString& stderrText);
Outcome outcome(QProcess* process, int code, QProcess::ExitStatus status);
void run(QObject* context, const QStringList& command, std::function<void(const Outcome&)> done,
    std::function<void(const QByteArray&)> output = { }, const QByteArray& input = { });

}
