#pragma once

#include "lamemail.h"

#include <QList>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QUrl>
#include <QVariantList>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>
#include <functional>
#include <utility>

class Mail : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QString path READ path NOTIFY changed)
    Q_PROPERTY(bool loading READ loading NOTIFY loadingChanged)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(QString subject READ subject NOTIFY changed)
    Q_PROPERTY(QString sender READ sender NOTIFY changed)
    Q_PROPERTY(QString recipients READ recipients NOTIFY changed)
    Q_PROPERTY(QString time READ time NOTIFY changed)
    Q_PROPERTY(QString account READ account NOTIFY changed)
    Q_PROPERTY(QString color READ color NOTIFY changed)
    Q_PROPERTY(QString kind READ kind NOTIFY changed)
    Q_PROPERTY(QString answerFrom READ answerFrom NOTIFY changed)
    Q_PROPERTY(QVariantList answerTo READ answerTo NOTIFY changed)
    Q_PROPERTY(QVariantList answerCc READ answerCc NOTIFY changed)
    Q_PROPERTY(QString answerSubject READ answerSubject NOTIFY changed)
    Q_PROPERTY(QString forwardSubject READ forwardSubject NOTIFY changed)
    Q_PROPERTY(QString quote READ quote NOTIFY changed)
    Q_PROPERTY(QString signature READ signature NOTIFY changed)
    Q_PROPERTY(QVariantMap draft READ draft NOTIFY changed)
    Q_PROPERTY(QVariantList parts READ parts NOTIFY changed)
    Q_PROPERTY(QVariantList attachments READ attachments NOTIFY changed)
    Q_PROPERTY(bool flagged READ flagged WRITE setFlagged NOTIFY flaggedChanged)
    Q_PROPERTY(QVariantList opening READ opening NOTIFY fetchingChanged)
    Q_PROPERTY(bool downloading READ downloading NOTIFY fetchingChanged)

public:
    using QObject::QObject;

    Q_INVOKABLE void open(const QString& path);
    Q_INVOKABLE void reload();
    Q_INVOKABLE void openAttachment(int index);
    Q_INVOKABLE void downloadAttachments(const QString& folder);

    QString path() const { return m_path; }
    bool loading() const { return m_loading; }
    QString error() const { return m_error; }
    QString subject() const { return m_shown.subject; }
    QString sender() const { return m_shown.sender; }
    QString recipients() const { return m_shown.recipients; }
    QString time() const { return m_shown.time; }
    QString account() const { return m_shown.account; }
    QString color() const { return m_shown.color; }
    QString kind() const { return m_shown.kind; }
    QString answerFrom() const { return m_shown.answerFrom; }
    QVariantList answerTo() const { return m_shown.answerTo; }
    QVariantList answerCc() const { return m_shown.answerCc; }
    QString answerSubject() const { return m_shown.answerSubject; }
    QString forwardSubject() const { return m_shown.forwardSubject; }
    QString quote() const { return m_shown.quote; }
    QString signature() const { return m_shown.signature; }
    QVariantMap draft() const { return m_shown.draft; }
    QVariantList parts() const { return m_shown.parts; }
    QVariantList attachments() const { return m_shown.attachments; }
    bool flagged() const { return m_shown.flagged; }
    void setFlagged(bool flagged);
    QVariantList opening() const;
    bool downloading() const;

signals:
    void changed();
    void loadingChanged();
    void flaggedChanged();
    void fetchingChanged();
    void attachmentReady(const QUrl& url);
    void attachmentsDownloaded(int count, const QString& folder);
    void attachmentFailed(const QString& error);

private:
    struct Shown {
        QString subject;
        QString sender;
        QString recipients;
        QString time;
        QString account;
        QString color;
        QString kind;
        QString answerFrom;
        QVariantList answerTo;
        QVariantList answerCc;
        QString answerSubject;
        QString forwardSubject;
        QString quote;
        QString signature;
        QVariantMap draft;
        QVariantList parts;
        QVariantList attachments;
        bool flagged = false;

        bool operator==(const Shown&) const = default;
    };

    static Shown parse(const QByteArray& json, QString* error);
    void show(Shown shown, const QString& error);
    void setLoading(bool loading);
    void fetch(int index, const QStringList& options,
        const std::function<void(const lamemail::Outcome&)>& done);

    QString m_path;
    QString m_error;
    Shown m_shown;
    QList<std::pair<QString, int>> m_fetching;
    bool m_loading = false;
    int m_generation = 0;
};
