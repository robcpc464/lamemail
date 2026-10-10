#pragma once

#include "entry.h"

#include <QAbstractListModel>
#include <QHash>
#include <QList>
#include <QString>
#include <QStringList>
#include <QtQml/qqmlregistration.h>

class Contacts : public QAbstractListModel {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(bool loading READ loading NOTIFY loadingChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(QString query READ query WRITE setQuery NOTIFY queryChanged)
    Q_PROPERTY(QStringList chosen READ chosen WRITE setChosen NOTIFY chosenChanged)

public:
    enum Role {
        NameRole = Qt::UserRole + 1,
        AddressRole,
        AccountRole,
        AccountColorRole,
        TimeRole,
        WrittenRole,
    };

    using QAbstractListModel::QAbstractListModel;

    int rowCount(const QModelIndex& parent = QModelIndex()) const override;
    QVariant data(const QModelIndex& index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    bool loading() const { return m_loading; }
    QString error() const { return m_error; }
    QString query() const { return m_query; }
    void setQuery(const QString& query);
    QStringList chosen() const { return m_chosen; }
    void setChosen(const QStringList& chosen);

    Q_INVOKABLE void reload();
    Q_INVOKABLE void arrange();
    Q_INVOKABLE QString accountOf(const QString& address) const;

signals:
    void loadingChanged();
    void errorChanged();
    void queryChanged();
    void chosenChanged();

private:
    void setLoading(bool loading);
    void setError(const QString& error);

    QList<Contact> m_all;
    QList<Contact> m_rows;
    QString m_query;
    QStringList m_chosen;
    bool m_loading = false;
    bool m_pending = false;
    QString m_error;
};
