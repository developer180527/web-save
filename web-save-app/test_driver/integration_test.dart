// Host side of `flutter drive` for integration_test/: it only relays the
// result reported by the app back to the command line.
import 'package:integration_test/integration_test_driver.dart';

Future<void> main() => integrationDriver();
