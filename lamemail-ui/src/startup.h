#pragma once

#include <QCommandLineOption>

class QQmlApplicationEngine;
class QQuickWindow;

namespace lamemail {

void identify();
QCommandLineOption dirOption();
QQuickWindow* open(QQmlApplicationEngine& engine, bool hidden);
void reveal(QQuickWindow* window, const QString& path);

}
