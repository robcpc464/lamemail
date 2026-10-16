#include "updates.h"

#include "lamemail.h"
#include "preferences.h"

#include <QCoreApplication>
#include <QDateTime>
#include <QSettings>

namespace {

const QString releaseKey = QStringLiteral("updates/release");
const QString checkedKey = QStringLiteral("updates/checked");
const QString dirKey = QStringLiteral("updates/dir");
constexpr int day = 24 * 60 * 60;
constexpr int hour = 60 * 60 * 1000;

std::optional<Release> cached()
{
    const QSettings settings;
    auto release = releaseFromJson(settings.value(releaseKey).toByteArray());
    if (release
        && (release->current != QCoreApplication::applicationVersion()
            || settings.value(dirKey).toString() != lamemail::dir()))
        return std::nullopt;
    return release;
}

}

Updates::Updates(QObject* parent)
    : QObject(parent)
    , m_release(cached())
{
    m_timer.setInterval(hour);
    connect(&m_timer, &QTimer::timeout, this, &Updates::check);
    m_timer.start();
    QTimer::singleShot(0, this, &Updates::check);
}

bool Updates::due() const
{
    const QDateTime checked = QSettings().value(checkedKey).toDateTime();
    return !m_release || !checked.isValid()
        || checked.secsTo(QDateTime::currentDateTimeUtc()) >= day;
}

void Updates::check()
{
    if (m_checking || !Preferences::checkUpdates() || !due())
        return;
    m_checking = true;
    lamemail::run(this,
        { QStringLiteral("update"), QStringLiteral("check"), QStringLiteral("--json") },
        [this](const lamemail::Outcome& outcome) {
            m_checking = false;
            const QByteArray line = outcome.out.trimmed();
            auto release = outcome.ok ? releaseFromJson(line) : std::nullopt;
            if (!release)
                return;
            QSettings settings;
            settings.setValue(releaseKey, line);
            settings.setValue(checkedKey, QDateTime::currentDateTimeUtc());
            settings.setValue(dirKey, lamemail::dir());
            m_release = std::move(release);
            emit changed();
        });
}
