// Cards must actually paint.
//
// They did not: a per-side border on a decoration that also had a
// borderRadius throws in BoxDecoration's painter, so every card rendered as
// an empty rounded rectangle. It looked like a hover effect, because hover
// set all four sides to one colour and the border became uniform — so the
// only card that ever drew its contents was the one under the pointer.
//
// The elevation tests passed throughout: they check colour and offset values,
// and never put a card on screen. This one pumps it.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_theme_engine/flutter_theme_engine.dart';
import 'package:websave_app/features/save_card.dart';
import 'package:websave_app/features/saves_controller.dart';
import 'package:websave_app/src/rust/api/vault_api.dart';
import 'package:websave_app/theme/packs.dart';
import 'package:websave_app/theme/websave_theme.dart';

class _OfflineController extends SavesController {
  @override
  Future<void> load({bool quiet = false}) async {}
}

SaveView _save() => const SaveView(
  id: 1,
  url: 'https://example.com',
  title: 'Example',
  description: 'A description',
  faviconUrl: '',
  thumbnail: '',
  favorite: false,
  isRead: false,
  status: 'active',
  tags: ['tag'],
  createdAt: 0,
);

Future<void> _pumpCard(WidgetTester tester, {bool selected = false}) async {
  final engine = ThemeEngine<WebSaveTheme>()..register(webSavePacks);
  await engine.restore();
  await tester.pumpWidget(
    ThemeEngineScope<WebSaveTheme>(
      engine: engine,
      child: MaterialApp(
        home: Scaffold(
          body: Center(
            child: SizedBox(
              width: SaveCard.width,
              height: SaveCard.heightFor(SaveCard.width),
              child: SaveCard(
                save: _save(),
                selected: selected,
                checking: false,
                thumbnailPath: null,
                onTap: () {},
                controller: _OfflineController(),
              ),
            ),
          ),
        ),
      ),
    ),
  );
}

void main() {
  testWidgets('an unhovered card paints its contents', (tester) async {
    await _pumpCard(tester);

    // The regression exactly: the decoration threw during paint, so the card
    // drew as an empty shape. tester.takeException() surfaces that.
    expect(
      tester.takeException(),
      isNull,
      reason: 'a paint-time exception leaves the card blank on screen while '
          'every value-level test still passes',
    );
    expect(find.text('Example'), findsOneWidget);
    expect(find.text('A description'), findsOneWidget);
  });

  testWidgets('a selected card paints too', (tester) async {
    await _pumpCard(tester, selected: true);
    expect(tester.takeException(), isNull);
    expect(find.text('Example'), findsOneWidget);
  });
}
