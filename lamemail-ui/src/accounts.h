#pragma once

#include "entry.h"

#include <QAbstractListModel>
#include <QHash>
#include <QList>
#include <QString>
#include <QStringList>
#include <QUrl>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>
#include <functional>

class Accounts : public QAbstractListModel {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(bool loading READ loading NOTIFY loadingChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(QStringList ids READ ids NOTIFY idsChanged)
    Q_PROPERTY(QVariantMap colors READ colors NOTIFY changed)
    Q_PROPERTY(QVariantMap senders READ senders NOTIFY changed)
    Q_PROPERTY(QVariantMap signatures READ signatures NOTIFY changed)

public:
    enum Role {
        IdRole = Qt::UserRole + 1,
        HostRole,
        PortRole,
        UsernameRole,
        SmtpHostRole,
        SmtpPortRole,
        CopyToSentRole,
        ColorRole,
        SignatureRole,
        ContextFolderRole,
        ArchiveByYearRole,
        PasswordRole,
        BusyRole,
        StatusRole,
        StatusOkRole,
        KindsRole,
        FoldersRole,
    };

    using QAbstractListModel::QAbstractListModel;

    int rowCount(const QModelIndex& parent = QModelIndex()) const override;
    QVariant data(const QModelIndex& index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    bool loading() const { return m_loading; }
    QString error() const { return m_error; }
    QStringList ids() const;
    QVariantMap colors() const { return byId(&Account::color); }
    QVariantMap senders() const { return byId(&Account::from); }
    QVariantMap signatures() const { return byId(&Account::signature); }

    Q_INVOKABLE void reload();
    Q_INVOKABLE void add(const QString& id, const QString& host, const QString& port,
        const QString& username, const QString& password, const QString& smtpHost,
        const QString& smtpPort);
    Q_INVOKABLE void test(const QString& id);
    Q_INVOKABLE void setSmtp(const QString& id, const QString& host, const QString& port);
    Q_INVOKABLE void setCopy(const QString& id, bool copyToSent);
    Q_INVOKABLE void setColor(const QString& id, const QString& color);
    Q_INVOKABLE void setSignature(const QString& id, const QString& signature);
    Q_INVOKABLE void setContextFolder(const QString& id, const QUrl& folder);
    Q_INVOKABLE void setArchiveByYear(const QString& id, bool byYear);
    Q_INVOKABLE void setPassword(const QString& id, const QString& password);
    Q_INVOKABLE void assignFolder(const QString& id, const QString& folder, const QString& kind,
        const QVariant& sync, bool makeDefault);
    Q_INVOKABLE void remove(const QString& id, bool purge);

signals:
    void loadingChanged();
    void errorChanged();
    void idsChanged();
    void changed();
    void addFinished(bool ok, const QString& id, const QString& message);
    void removed(const QString& id, const QString& message);
    void passwordStored(const QString& id);
    void folderAssigned(const QString& id);

private:
    struct Status {
        bool busy = false;
        bool ok = false;
        QString text;
    };

    QVariantMap byId(QString Account::* field) const;
    void act(const QString& id, const QStringList& command, const QByteArray& input,
        bool reloadAfter, std::function<void(bool ok, const QString& line)> done = { });
    void setStatus(const QString& id, const Status& status);
    void replace(QList<Account> rows);
    void setLoading(bool loading);
    void setError(const QString& error);

    QList<Account> m_rows;
    QHash<QString, Status> m_status;
    bool m_loading = false;
    bool m_pending = false;
    QString m_error;
};
