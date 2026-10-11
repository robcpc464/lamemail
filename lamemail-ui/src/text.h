#pragma once

#include "entry.h"
#include "texts.h"

#include <QDateTime>
#include <QString>
#include <QStringList>
#include <QVariantMap>
#include <optional>

struct LinkedText {
    QString html;
    QStringList shortened;
};

QString person(const Addr& addr);
QString displayName(const Addr& addr);
QString personMarkup(const Addr& addr);
QString people(const QList<Addr>& list, QString (*format)(const Addr&));
std::optional<QDateTime> parseTime(const QString& received);
QString shortTime(const QDateTime& when, const QDateTime& now, Lang lang);
QString detailTime(const QDateTime& when, const QDateTime& now, Lang lang);
QString dateText(const QDate& date, const char* format, Lang lang);
int dayIndex(const std::optional<QDateTime>& when, const QDate& today);
QString fileSize(qint64 bytes);
QString tildePath(const QString& path);
LinkedText linkedText(const QString& text);
QVariantMap aiReport(const Suggestion& suggestion, const QDateTime& now, Lang lang);
