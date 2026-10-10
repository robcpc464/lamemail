#pragma once

#include "entry.h"

#include <QAbstractListModel>
#include <QList>
#include <QString>
#include <QStringList>
#include <QtQml/qqmlregistration.h>

class Folders : public QAbstractListModel {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(bool loading READ loading NOTIFY loadingChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)

public:
    enum Role {
        KindRole = Qt::UserRole + 1,
        IconRole,
        DenseRole,
        DimRole,
        StrikeRole,
        ItalicRole,
        IndentRole,
        ColorRole,
        IconSetRole,
        DenseSetRole,
        DimSetRole,
        StrikeSetRole,
        ItalicSetRole,
        IndentSetRole,
        ColorSetRole,
        AnySetRole,
        MessagesRole,
        UnreadRole,
        ExistsRole,
    };

    using QAbstractListModel::QAbstractListModel;

    int rowCount(const QModelIndex& parent = QModelIndex()) const override;
    QVariant data(const QModelIndex& index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    bool loading() const { return m_loading; }
    QString error() const { return m_error; }

    Q_INVOKABLE void reload();
    Q_INVOKABLE void set(const QString& kind, const QString& property, const QString& value);
    Q_INVOKABLE void reset(const QString& kind);

signals:
    void loadingChanged();
    void errorChanged();

private:
    void change(const QStringList& command);
    void runChange();
    void replace(QList<Folder> rows);
    void setLoading(bool loading);
    void setError(const QString& error);

    QList<Folder> m_rows;
    QList<QStringList> m_changes;
    bool m_loading = false;
    bool m_pending = false;
    QString m_error;
};
