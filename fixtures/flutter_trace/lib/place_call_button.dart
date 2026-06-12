import 'package:http/http.dart' as http;

Future<void> placeCall() async {
  await http.post(Uri.parse('/api/calls'));
}

class PlaceCallButton {
  PlaceCallButton();

  ElevatedButton build() {
    return ElevatedButton(onPressed: placeCall, child: const Text('Place Call'));
  }
}

class ElevatedButton {
  const ElevatedButton({required this.onPressed, required this.child});
  final void Function() onPressed;
  final Text child;
}

class Text {
  const Text(String label);
}

abstract class Widget {}
