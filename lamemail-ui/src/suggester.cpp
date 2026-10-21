#include "suggester.h"

#include "entry.h"
#include "lamemail.h"
#include "text.h"

#include <csignal>
#include <sys/prctl.h>
#include <unistd.h>

namespace {

void terminateGroup(const QProcess* process)
{
    if (process->processId() > 0)
        ::kill(-pid_t(process->processId()), SIGTERM);
}

}

Suggester::~Suggester()
{
    if (!m_process)
        return;
    terminateGroup(m_process);
    m_process->waitForFinished(2000);
}

void Suggester::suggest(const QVariantMap& request)
{
    if (m_process)
        return;
    const auto value = [&](const char* key) { return request.value(QLatin1String(key)); };
    QStringList command { QStringLiteral("ai"), QStringLiteral("suggest"),
        QStringLiteral("--json") };
    for (const char* kind : { "to", "cc" }) {
        for (const QString& address : value(kind).toStringList())
            command.append(QStringLiteral("--%1=%2").arg(QLatin1String(kind), address));
    }
    if (value("written").toBool())
        command.append(QStringLiteral("--no-questions"));
    command << QStringLiteral("--subject=") + value("subject").toString()
            << QStringLiteral("--before=") + value("before").toString()
            << QStringLiteral("--after=") + value("after").toString();
    const QString reply = value("reply").toString();
    if (reply.isEmpty())
        command << QStringLiteral("--") << value("account").toString();
    else
        command.append(QStringLiteral("--reply=") + reply);
    auto* process = new QProcess(this);
    m_process = process;
    process->setChildProcessModifier([] {
        ::setpgid(0, 0);
        ::prctl(PR_SET_PDEATHSIG, SIGTERM);
    });
    connect(process, &QProcess::finished, this, &Suggester::finish);
    connect(process, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (error != QProcess::FailedToStart)
            return;
        const QString message
            = lamemail::binary() + QStringLiteral(": ") + m_process->errorString();
        release()->deleteLater();
        emit failed(message);
    });
    process->start(lamemail::binary(), lamemail::arguments(command));
    process->write(value("instruction").toString().toUtf8());
    process->closeWriteChannel();
    emit runningChanged();
}

void Suggester::cancel()
{
    if (!m_process)
        return;
    QProcess* process = release();
    terminateGroup(process);
    connect(process, &QProcess::finished, process, &QObject::deleteLater);
}

void Suggester::finish(int code, QProcess::ExitStatus status)
{
    QProcess* process = release();
    process->deleteLater();
    const lamemail::Outcome outcome = lamemail::outcome(process, code, status);
    if (!outcome.ok) {
        emit failed(outcome.error);
        return;
    }
    QString error;
    const auto suggestion = suggestionFromJson(outcome.out.trimmed(), &error);
    if (!suggestion) {
        emit failed(error);
        return;
    }
    if (!suggestion->question.isEmpty())
        emit asked(suggestion->question);
    else
        emit suggested(suggestion->text);
    emit reported(aiReport(*suggestion, QDateTime::currentDateTime(), lang()));
}

QProcess* Suggester::release()
{
    QProcess* process = std::exchange(m_process, nullptr);
    process->disconnect(this);
    emit runningChanged();
    return process;
}
