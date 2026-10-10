#include "accounts.h"

#include "lamemail.h"
#include "text.h"

namespace {

QString lastOutputLine(const QByteArray& out)
{
    return lamemail::lastLine(QString::fromUtf8(out)).value_or(QString());
}

QString boolText(bool on) { return on ? QStringLiteral("true") : QStringLiteral("false"); }

}

int Accounts::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : int(m_rows.size());
}

QVariant Accounts::data(const QModelIndex& index, int role) const
{
    if (!index.isValid() || index.row() >= m_rows.size())
        return { };
    const Account& row = m_rows.at(index.row());
    switch (role) {
    case IdRole:
        return row.id;
    case HostRole:
        return row.host;
    case PortRole:
        return row.port;
    case UsernameRole:
        return row.username;
    case SmtpHostRole:
        return row.smtpHost;
    case SmtpPortRole:
        return row.smtpPort;
    case CopyToSentRole:
        return row.copyToSent;
    case ColorRole:
        return row.color;
    case SignatureRole:
        return row.signature;
    case ContextFolderRole:
        return tildePath(row.contextFolder);
    case ArchiveByYearRole:
        return row.archiveByYear;
    case PasswordRole:
        return row.password;
    case BusyRole:
        return m_status.value(row.id).busy;
    case StatusRole:
        return m_status.value(row.id).text;
    case StatusOkRole:
        return m_status.value(row.id).ok;
    case KindsRole:
        return row.kinds;
    case FoldersRole:
        return row.folders;
    default:
        return { };
    }
}

QHash<int, QByteArray> Accounts::roleNames() const
{
    return {
        { IdRole, "name" },
        { HostRole, "host" },
        { PortRole, "port" },
        { UsernameRole, "username" },
        { SmtpHostRole, "smtpHost" },
        { SmtpPortRole, "smtpPort" },
        { CopyToSentRole, "copyToSent" },
        { ColorRole, "color" },
        { SignatureRole, "signature" },
        { ContextFolderRole, "contextFolder" },
        { ArchiveByYearRole, "archiveByYear" },
        { PasswordRole, "password" },
        { BusyRole, "busy" },
        { StatusRole, "status" },
        { StatusOkRole, "statusOk" },
        { KindsRole, "kinds" },
        { FoldersRole, "folders" },
    };
}

QStringList Accounts::ids() const
{
    QStringList ids;
    for (const Account& row : m_rows)
        ids.append(row.id);
    return ids;
}

QVariantMap Accounts::byId(QString Account::* field) const
{
    QVariantMap values;
    for (const Account& row : m_rows)
        values.insert(row.id, row.*field);
    return values;
}

void Accounts::reload()
{
    if (m_loading) {
        m_pending = true;
        return;
    }
    m_pending = false;
    setLoading(true);
    lamemail::run(this,
        { QStringLiteral("account"), QStringLiteral("list"), QStringLiteral("--json") },
        [this](const lamemail::Outcome& outcome) {
            if (outcome.ok) {
                replace(fromJsonLines(outcome.out, accountFromJson));
                setError({ });
            } else {
                setError(outcome.error);
            }
            setLoading(false);
            if (m_pending)
                reload();
        });
}

void Accounts::add(const QString& id, const QString& host, const QString& port,
    const QString& username, const QString& password, const QString& smtpHost,
    const QString& smtpPort)
{
    QStringList command { QStringLiteral("account"), QStringLiteral("add"),
        QStringLiteral("--host"), host, QStringLiteral("--user"), username };
    if (!port.isEmpty())
        command << QStringLiteral("--port") << port;
    if (!smtpHost.isEmpty())
        command << QStringLiteral("--smtp-host") << smtpHost;
    if (!smtpHost.isEmpty() && !smtpPort.isEmpty())
        command << QStringLiteral("--smtp-port") << smtpPort;
    command << QStringLiteral("--") << id;
    lamemail::run(
        this, command,
        [this, id](const lamemail::Outcome& outcome) {
            emit addFinished(
                outcome.ok, id, outcome.ok ? lastOutputLine(outcome.out) : outcome.error);
            if (outcome.ok)
                reload();
        },
        { }, password.toUtf8());
}

void Accounts::test(const QString& id)
{
    act(id, { QStringLiteral("account"), QStringLiteral("test"), id }, { }, false);
}

void Accounts::setSmtp(const QString& id, const QString& host, const QString& port)
{
    QStringList command { QStringLiteral("account"), QStringLiteral("smtp"), id,
        QStringLiteral("--host"), host };
    if (!port.isEmpty())
        command << QStringLiteral("--port") << port;
    act(id, command, { }, true);
}

void Accounts::setCopy(const QString& id, bool copyToSent)
{
    act(id,
        { QStringLiteral("account"), QStringLiteral("smtp"), id, QStringLiteral("--copy"),
            boolText(copyToSent) },
        { }, true);
}

void Accounts::setColor(const QString& id, const QString& color)
{
    act(id, { QStringLiteral("account"), QStringLiteral("color"), id, color }, { }, true);
}

void Accounts::setSignature(const QString& id, const QString& signature)
{
    act(id, { QStringLiteral("account"), QStringLiteral("signature"), id }, signature.toUtf8(),
        true);
}

void Accounts::setContextFolder(const QString& id, const QUrl& folder)
{
    QStringList command { QStringLiteral("account"), QStringLiteral("context"), id };
    if (folder.isLocalFile())
        command.append(folder.toLocalFile());
    act(id, command, { }, true);
}

void Accounts::setArchiveByYear(const QString& id, bool byYear)
{
    act(id,
        { QStringLiteral("account"), QStringLiteral("archive"), id, QStringLiteral("--by-year"),
            boolText(byYear) },
        { }, true);
}

void Accounts::setPassword(const QString& id, const QString& password)
{
    act(id, { QStringLiteral("account"), QStringLiteral("password"), id }, password.toUtf8(), true,
        [this, id](bool ok, const QString&) {
            if (ok)
                emit passwordStored(id);
        });
}

void Accounts::assignFolder(const QString& id, const QString& folder, const QString& kind,
    const QVariant& sync, bool makeDefault)
{
    QStringList command { QStringLiteral("account"), QStringLiteral("folder") };
    if (sync.isValid())
        command << QStringLiteral("--sync") << boolText(sync.toBool());
    if (makeDefault)
        command.append(QStringLiteral("--default"));
    command << QStringLiteral("--") << id << folder << kind;
    act(id, command, { }, true, [this, id, makeDefault](bool ok, const QString&) {
        if (ok && !makeDefault)
            emit folderAssigned(id);
    });
}

void Accounts::remove(const QString& id, bool purge)
{
    QStringList command { QStringLiteral("account"), QStringLiteral("remove"), id };
    if (purge)
        command.append(QStringLiteral("--purge"));
    act(id, command, { }, true, [this, id](bool ok, const QString& line) {
        if (!ok)
            return;
        m_status.remove(id);
        emit removed(id, line);
    });
}

void Accounts::act(const QString& id, const QStringList& command, const QByteArray& input,
    bool reloadAfter, std::function<void(bool ok, const QString& line)> done)
{
    if (m_status.value(id).busy)
        return;
    setStatus(id, { .busy = true, .ok = false, .text = { } });
    lamemail::run(
        this, command,
        [this, id, reloadAfter, done](const lamemail::Outcome& outcome) {
            const QString line = outcome.ok ? lastOutputLine(outcome.out) : outcome.error;
            setStatus(id, { .busy = false, .ok = outcome.ok, .text = line });
            if (done)
                done(outcome.ok, line);
            if (reloadAfter)
                reload();
        },
        { }, input);
}

void Accounts::setStatus(const QString& id, const Status& status)
{
    m_status.insert(id, status);
    for (qsizetype row = 0; row < m_rows.size(); ++row) {
        if (m_rows.at(row).id == id)
            emit dataChanged(
                index(int(row)), index(int(row)), { BusyRole, StatusRole, StatusOkRole });
    }
}

void Accounts::replace(QList<Account> rows)
{
    if (rows == m_rows)
        return;
    bool sameIds = rows.size() == m_rows.size();
    for (qsizetype i = 0; sameIds && i < rows.size(); ++i)
        sameIds = rows.at(i).id == m_rows.at(i).id;
    if (!sameIds) {
        beginResetModel();
        m_rows = std::move(rows);
        endResetModel();
        emit changed();
        emit idsChanged();
        return;
    }
    for (qsizetype i = 0; i < rows.size(); ++i) {
        if (rows.at(i) == m_rows.at(i))
            continue;
        m_rows[i] = rows.at(i);
        emit dataChanged(index(int(i)), index(int(i)));
    }
    emit changed();
}

void Accounts::setLoading(bool loading)
{
    if (m_loading == loading)
        return;
    m_loading = loading;
    emit loadingChanged();
}

void Accounts::setError(const QString& error)
{
    if (m_error == error)
        return;
    m_error = error;
    emit errorChanged();
}
