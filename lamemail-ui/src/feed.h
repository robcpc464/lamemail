#pragma once

#include "entry.h"

#include <QAbstractListModel>
#include <QByteArray>
#include <QDateTime>
#include <QElapsedTimer>
#include <QHash>
#include <QList>
#include <QString>
#include <QStringList>
#include <QtQml/qqmlregistration.h>
#include <optional>
#include <vector>

class Feed : public QAbstractListModel {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(int count READ count NOTIFY countChanged)
    Q_PROPERTY(int unread READ unread NOTIFY unreadChanged)
    Q_PROPERTY(bool loading READ loading NOTIFY loadingChanged)
    Q_PROPERTY(bool streaming READ streaming NOTIFY streamingChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(QString query READ query WRITE setQuery NOTIFY queryChanged)
    Q_PROPERTY(bool flaggedFirst READ flaggedFirst WRITE setFlaggedFirst NOTIFY flaggedFirstChanged)
    Q_PROPERTY(QStringList kinds READ kinds WRITE setKinds NOTIFY kindsChanged)

public:
    enum Role {
        AccountRole = Qt::UserRole + 1,
        AccountColorRole,
        IconRole,
        DenseRole,
        DimRole,
        StrikeRole,
        ItalicRole,
        IndentRole,
        StyleColorRole,
        SenderRole,
        RecipientRole,
        MoreRecipientsRole,
        OtherRecipientsRole,
        SubjectRole,
        TimeRole,
        SeenRole,
        FlaggedRole,
        AnsweredRole,
        ForwardedRole,
        FreshRole,
        AttachmentsRole,
        PathRole,
        DayRole,
        KindRole,
    };

    explicit Feed(QObject* parent = nullptr);

    int rowCount(const QModelIndex& parent = QModelIndex()) const override;
    QVariant data(const QModelIndex& index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    int count() const { return m_count; }
    int unread() const { return m_unread; }
    bool loading() const { return m_loading; }
    bool streaming() const { return m_streaming; }
    QString error() const { return m_error; }
    QString query() const { return m_query; }
    void setQuery(const QString& query);
    bool flaggedFirst() const { return m_flaggedFirst; }
    void setFlaggedFirst(bool flaggedFirst);
    QStringList kinds() const { return m_kinds; }
    void setKinds(const QStringList& kinds);

    Q_INVOKABLE void reload();
    Q_INVOKABLE void refreshTimes();
    Q_INVOKABLE void markSeen(const QString& path, bool seen);
    Q_INVOKABLE void markFlagged(const QString& path, bool flagged);
    Q_INVOKABLE void move(const QString& path, const QString& folder);
    Q_INVOKABLE QString subject(const QString& path) const;

signals:
    void countChanged();
    void unreadChanged();
    void loadingChanged();
    void streamingChanged();
    void errorChanged();
    void queryChanged();
    void flaggedFirstChanged();
    void kindsChanged();
    void actionFailed(const QString& path, const QString& error);
    void moved(
        const QString& path, const QString& from, const QString& folder, const QString& target);

private:
    struct Row {
        Entry entry;
        QString sender;
        QString recipient;
        QString others;
        int more = 0;
        std::optional<QDateTime> when;
        QString time;
        int day = 2;
        qint64 insertedAt = -1;
    };

    struct Mark {
        std::optional<bool> seen;
        std::optional<bool> flagged;

        void applyTo(Flags& flags) const;
    };

    struct Job {
        QString path;
        QStringList command;
    };

    QStringList listArguments() const;
    void mark(const QString& path, const Mark& change, const QString& argument);
    void enqueue(const Job& job);
    void runJob();
    void settleMove(const QString& target, const QByteArray& out);
    void read(const QByteArray& chunk, bool final);
    void append(const QList<Entry>& batch);
    void merge(const QList<Entry>& incoming, bool fresh);
    void insert(qsizetype at, std::vector<Row> rows);
    void remove(qsizetype start, qsizetype end);
    qsizetype indexOf(const QString& path, qsizetype from) const;
    Row row(const Entry& entry, const QDateTime& now, bool fresh) const;
    bool fresh(const Row& row) const;
    void updateCounts();
    void setLoading(bool loading);
    void setStreaming(bool streaming);
    void setError(const QString& error);

    std::vector<Row> m_rows;
    QList<Job> m_jobs;
    QList<Entry> m_incoming;
    QByteArray m_buffer;
    QElapsedTimer m_clock;
    int m_count = 0;
    int m_unread = 0;
    bool m_started = false;
    bool m_loading = false;
    bool m_streaming = false;
    bool m_pending = false;
    bool m_reloadAfterJobs = false;
    bool m_flaggedFirst = false;
    QString m_error;
    QString m_query;
    QStringList m_kinds;
    QStringList m_shown;
    QDate m_today;
};
