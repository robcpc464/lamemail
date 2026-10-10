#include "mailwatch.h"

#include "entry.h"
#include "lamemail.h"
#include "text.h"

#include <QDebug>
#include <QVariantMap>
#include <csignal>
#include <sys/prctl.h>

namespace {

constexpr int againMs = 60000;
constexpr int stopMs = 1000;

QVariantMap mailOf(const Arrival& arrival)
{
    return {
        { QStringLiteral("path"), arrival.path },
        { QStringLiteral("sender"),
            arrival.from.isEmpty() ? QString() : displayName(arrival.from.constFirst()) },
        { QStringLiteral("subject"), arrival.subject },
    };
}

}

MailWatch::MailWatch(QObject* parent)
    : QObject(parent)
{
    m_again.setSingleShot(true);
    m_again.setInterval(againMs);
    connect(&m_again, &QTimer::timeout, this, &MailWatch::start);
}

MailWatch::~MailWatch()
{
    if (!m_process)
        return;
    m_process->disconnect(this);
    m_process->terminate();
    m_process->waitForFinished(stopMs);
}

QString MailWatch::failures() const
{
    QStringList lines;
    for (auto failure = m_failures.cbegin(); failure != m_failures.cend(); ++failure)
        lines.append(failure.key() + QStringLiteral(": ") + failure.value());
    return lines.join(QLatin1Char('\n'));
}

void MailWatch::start()
{
    if (m_process)
        return;
    m_again.stop();
    m_pending.clear();
    if (!m_failures.isEmpty()) {
        m_failures.clear();
        emit stateChanged();
    }
    auto* process = new QProcess(this);
    m_process = process;
    process->setChildProcessModifier([] { ::prctl(PR_SET_PDEATHSIG, SIGTERM); });
    process->setProcessChannelMode(QProcess::ForwardedErrorChannel);
    connect(process, &QProcess::readyReadStandardOutput, this, &MailWatch::read);
    connect(process, &QProcess::finished, this, &MailWatch::finish);
    connect(process, &QProcess::errorOccurred, this, [this, process](QProcess::ProcessError error) {
        if (error != QProcess::FailedToStart)
            return;
        qWarning().noquote() << "lamemail-ui: lamemail watch:" << process->errorString();
        process->deleteLater();
        m_process = nullptr;
    });
    process->start(lamemail::binary(),
        lamemail::arguments({ QStringLiteral("watch"), QStringLiteral("--json") }));
    process->closeWriteChannel();
}

void MailWatch::restart()
{
    stop();
    start();
}

void MailWatch::stop()
{
    m_again.stop();
    if (!m_process)
        return;
    QProcess* process = std::exchange(m_process, nullptr);
    process->disconnect(this);
    connect(process, &QProcess::finished, process, &QObject::deleteLater);
    process->terminate();
    settle();
}

void MailWatch::settle()
{
    if (m_syncing.isEmpty() && m_progress.isEmpty())
        return;
    m_syncing.clear();
    m_progress.clear();
    emit stateChanged();
}

void MailWatch::read()
{
    m_pending += m_process->readAllStandardOutput();
    for (qsizetype end = m_pending.indexOf('\n'); end >= 0; end = m_pending.indexOf('\n')) {
        const QByteArray line = m_pending.left(end);
        m_pending.remove(0, end + 1);
        handle(line);
    }
}

void MailWatch::handle(const QByteArray& line)
{
    if (line.trimmed().isEmpty())
        return;
    QString problem;
    const auto parsed = watchLineFromJson(line, &problem);
    if (!parsed) {
        qWarning().noquote() << "lamemail-ui:" << problem;
        return;
    }
    if (parsed->progress) {
        m_progress = parsed->progress->toMap();
        emit stateChanged();
        return;
    }
    if (parsed->syncing) {
        if (!m_syncing.contains(parsed->account)) {
            m_syncing.append(parsed->account);
            emit stateChanged();
        }
        return;
    }
    bool wasSyncing = m_syncing.removeAll(parsed->account) > 0;
    if (m_progress.value(QStringLiteral("account")).toString() == parsed->account) {
        m_progress.clear();
        wasSyncing = true;
    }
    if (!parsed->error.isEmpty()) {
        m_failures.insert(parsed->account, parsed->error);
        emit stateChanged();
        const QString message = parsed->account + QStringLiteral(": ") + parsed->error;
        if (parsed->retrying)
            qWarning().noquote() << "lamemail-ui: lamemail watch:" << message;
        else
            emit stopped(message);
        return;
    }
    if (m_failures.remove(parsed->account) > 0 || wasSyncing)
        emit stateChanged();
    emit synced();
    if (parsed->arrived.isEmpty())
        return;
    QVariantList mails;
    for (const Arrival& arrival : parsed->arrived)
        mails.append(mailOf(arrival));
    emit arrived(mails);
}

void MailWatch::finish(int, QProcess::ExitStatus status)
{
    read();
    m_process->deleteLater();
    m_process = nullptr;
    settle();
    if (status != QProcess::CrashExit)
        return;
    qWarning().noquote() << "lamemail-ui: lamemail watch crashed, it starts again in a minute";
    m_again.start();
}
